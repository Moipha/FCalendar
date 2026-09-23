use super::client::CaldavClient;
use super::multistatus::{parse_multistatus, prop_text, supports_component};
use url::Url;

#[derive(Debug, Clone)]
pub struct DiscoveredCalendar {
    pub href: String,
    pub display_name: String,
    pub color: Option<String>,
    pub supports_vevent: bool,
    pub supports_vtodo: bool,
    pub ctag: Option<String>,
    pub sync_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DiscoverResult {
    pub principal_url: String,
    pub calendar_home_url: String,
    pub calendars: Vec<DiscoveredCalendar>,
}

const PROPFIND_PRINCIPAL: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:current-user-principal/>
  </D:prop>
</D:propfind>"#;

const PROPFIND_HOME: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <C:calendar-home-set/>
  </D:prop>
</D:propfind>"#;

const PROPFIND_CALENDARS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav" xmlns:ICAL="http://apple.com/ns/ical/">
  <D:prop>
    <D:resourcetype/>
    <D:displayname/>
    <ICAL:calendar-color/>
    <C:supported-calendar-component-set/>
    <C:getctag/>
    <D:sync-token/>
  </D:prop>
</D:propfind>"#;

pub async fn discover(
    server_url: &str,
    username: &str,
    password: &str,
) -> Result<DiscoverResult, String> {
    let client = CaldavClient::new(username, password)?;
    let base = normalize_server_url(server_url, username)?;
    let origin = base.origin().ascii_serialization();

    let start_url = if let Ok(well_known) = Url::parse(&format!("{origin}/.well-known/caldav")) {
        well_known.to_string()
    } else {
        base.to_string()
    };

    let principal_href = find_principal(&client, &start_url, &base).await?;
    let principal_url = resolve_href(&base, &principal_href)?;

    let home_href = find_calendar_home(&client, principal_url.as_str()).await?;
    let calendar_home_url = resolve_href(&base, &home_href)?;

    let calendars = list_calendars(&client, calendar_home_url.as_str()).await?;

    Ok(DiscoverResult {
        principal_url: principal_url.to_string(),
        calendar_home_url: calendar_home_url.to_string(),
        calendars,
    })
}

async fn find_principal(
    client: &CaldavClient,
    start: &str,
    fallback_base: &Url,
) -> Result<String, String> {
    if let Ok(xml) = client.propfind(start, 0, PROPFIND_PRINCIPAL).await {
        if let Some(href) = extract_first_href_prop(&xml, &["current-user-principal"]) {
            return Ok(href);
        }
    }

    let user_root = fallback_base.clone();
    let xml = client
        .propfind(user_root.as_str(), 0, PROPFIND_PRINCIPAL)
        .await?;
    extract_first_href_prop(&xml, &["current-user-principal"])
        .ok_or_else(|| "未找到 current-user-principal".into())
}

async fn find_calendar_home(client: &CaldavClient, principal_url: &str) -> Result<String, String> {
    let xml = client
        .propfind(principal_url, 0, PROPFIND_HOME)
        .await?;
    extract_first_href_prop(&xml, &["calendar-home-set"])
        .ok_or_else(|| "未找到 calendar-home-set".into())
}

async fn list_calendars(
    client: &CaldavClient,
    calendar_home_url: &str,
) -> Result<Vec<DiscoveredCalendar>, String> {
    let xml = client
        .propfind(calendar_home_url, 1, PROPFIND_CALENDARS)
        .await?;
    let items = parse_multistatus(&xml)?;
    let home_norm = normalize_path(calendar_home_url);

    let mut out = Vec::new();
    for item in items {
        let is_calendar = item.props.is_calendar
            || supports_component(&item.props, "VEVENT")
            || supports_component(&item.props, "VTODO");
        if !is_calendar {
            continue;
        }
        let item_path = normalize_path(&item.href);
        if item_path == home_norm || item_path.ends_with(".ics") {
            continue;
        }
        let display_name = prop_text(&item.props, &["displayname"])
            .unwrap_or_else(|| item.href.rsplit('/').next().unwrap_or("日历").into());
        out.push(DiscoveredCalendar {
            href: item.href.clone(),
            display_name,
            color: prop_text(&item.props, &["calendar-color"]),
            supports_vevent: supports_component(&item.props, "VEVENT"),
            supports_vtodo: supports_component(&item.props, "VTODO"),
            ctag: prop_text(&item.props, &["getctag"]),
            sync_token: prop_text(&item.props, &["sync-token"]),
        });
    }
    Ok(out)
}

fn extract_first_href_prop(xml: &str, prop_names: &[&str]) -> Option<String> {
    let items = parse_multistatus(xml).ok()?;
    for item in items {
        for name in prop_names {
            if let Some(href) = item.props.values.get(*name) {
                if !href.is_empty() {
                    return Some(href.clone());
                }
            }
        }
    }
    None
}

pub fn normalize_server_url(server_url: &str, username: &str) -> Result<Url, String> {
    let trimmed = server_url.trim();
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let mut url = Url::parse(&with_scheme).map_err(|e| format!("服务器地址无效: {e}"))?;
    if url.path() == "/" || url.path().is_empty() {
        url.set_path(&format!("/{}/", username.trim_matches('/')));
    } else if !url.path().ends_with('/') {
        url.path_segments_mut()
            .map_err(|_| "服务器地址无效".to_string())?
            .push("");
    }
    Ok(url)
}

/// 把集合或对象 href 拼成 reqwest 可用的绝对 URL。
pub fn absolute_url(base: &Url, href: &str) -> Result<String, String> {
    Ok(resolve_href(base, href)?.to_string())
}

/// 集合目录 URL，保证以 `/` 结尾，避免 `join("x.ics")` 把最后一段目录名换掉。
pub fn collection_url(base: &Url, href: &str) -> Result<String, String> {
    Ok(ensure_trailing_slash(resolve_href(base, href)?).to_string())
}

pub fn resolve_href(base: &Url, href: &str) -> Result<Url, String> {
    if href.starts_with("http://") || href.starts_with("https://") {
        return Url::parse(href).map_err(|e| format!("解析 href: {e}"));
    }
    base.join(href)
        .map_err(|e| format!("拼接 href {href}: {e}"))
}

fn ensure_trailing_slash(mut url: Url) -> Url {
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    url
}

fn normalize_path(href: &str) -> String {
    let path = if let Ok(u) = Url::parse(href) {
        u.path().trim_end_matches('/').to_string()
    } else {
        href.trim_end_matches('/').to_string()
    };
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_adds_scheme_and_user_path() {
        let url = normalize_server_url("localhost:5232", "alice").unwrap();
        assert_eq!(url.scheme(), "http");
        assert!(url.path().contains("alice"));
    }

    #[test]
    fn absolute_url_keeps_host_from_server() {
        let base = Url::parse("http://192.168.1.8:5232/alice/").unwrap();
        assert_eq!(
            absolute_url(&base, "/alice/work/").unwrap(),
            "http://192.168.1.8:5232/alice/work/"
        );
    }

    #[test]
    fn collection_url_adds_trailing_slash() {
        let base = Url::parse("http://192.168.1.8:5232/alice/").unwrap();
        assert_eq!(
            collection_url(&base, "/alice/work").unwrap(),
            "http://192.168.1.8:5232/alice/work/"
        );
    }
}
