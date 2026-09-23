use super::client::CaldavClient;
use super::discover::absolute_url;
use super::multistatus::{parse_multistatus, prop_text};
use url::Url;

const PROPFIND_OBJECTS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:getetag/>
    <C:calendar-data/>
    <D:resourcetype/>
  </D:prop>
</D:propfind>"#;

#[derive(Debug, Clone)]
pub struct CollectionObject {
    pub href: String,
    pub etag: Option<String>,
    pub ics: String,
}

pub async fn list_collection_objects(
    client: &CaldavClient,
    calendar_url: &str,
) -> Result<Vec<CollectionObject>, String> {
    let xml = client.propfind(calendar_url, 1, PROPFIND_OBJECTS).await?;
    let items = parse_multistatus(&xml)?;
    let base = Url::parse(calendar_url).map_err(|e| format!("解析集合 URL: {e}"))?;
    let mut out = Vec::new();
    for item in items {
        if item.props.is_calendar {
            continue;
        }
        if item.href.ends_with('/') {
            continue;
        }
        let etag = prop_text(&item.props, &["getetag"]);
        let mut ics = prop_text(&item.props, &["calendar-data"]).unwrap_or_default();
        if !ics.contains("BEGIN:") {
            let url = absolute_url(&base, &item.href)?;
            match client.get(&url).await {
                Ok(text) if text.contains("BEGIN:") => ics = text,
                Err(e) => {
                    eprintln!("GET {} 补日历数据失败: {e}", item.href);
                    continue;
                }
                _ => continue,
            }
        }
        out.push(CollectionObject {
            href: item.href,
            etag,
            ics,
        });
    }
    Ok(out)
}
