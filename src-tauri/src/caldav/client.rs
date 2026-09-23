use reqwest::Client;
use std::time::Duration;

pub struct CaldavClient {
    http: Client,
    username: String,
    password: String,
}

impl CaldavClient {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Result<Self, String> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|e| format!("创建 HTTP 客户端: {e}"))?;
        Ok(Self {
            http,
            username: username.into(),
            password: password.into(),
        })
    }

    pub async fn propfind(&self, url: &str, depth: u8, body: &str) -> Result<String, String> {
        let response = self
            .http
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", depth.to_string())
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| format!("PROPFIND 请求失败: {e}"))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("读取响应: {e}"))?;

        if !status.is_success() {
            return Err(format!(
                "PROPFIND {} 返回 {}: {}",
                url,
                status.as_u16(),
                truncate_err(&text)
            ));
        }
        Ok(text)
    }

    pub async fn get(&self, url: &str) -> Result<String, String> {
        let response = self
            .http
            .get(url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| format!("GET 失败: {e}"))?;
        let status = response.status();
        let text = response.text().await.map_err(|e| format!("读取 GET: {e}"))?;
        if !status.is_success() {
            return Err(format!("GET {} 返回 {}", url, status.as_u16()));
        }
        Ok(text)
    }

    pub async fn put(
        &self,
        url: &str,
        body: &str,
        if_match: Option<&str>,
    ) -> Result<(u16, Option<String>, String), String> {
        let mut req = self
            .http
            .put(url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "text/calendar; charset=utf-8")
            .body(body.to_string());
        if let Some(etag) = if_match {
            req = req.header("If-Match", etag);
        }
        let response = req.send().await.map_err(|e| format!("PUT 失败: {e}"))?;
        let status = response.status().as_u16();
        let etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let text = response.text().await.unwrap_or_default();
        if status >= 400 {
            let msg = format_http_err("PUT", url, status, &text);
            eprintln!("{msg}");
            eprintln!("PUT ICS 预览: {}", preview_ics(body));
        }
        Ok((status, etag, text))
    }

    pub async fn delete(&self, url: &str, if_match: Option<&str>) -> Result<(u16, String), String> {
        let mut req = self
            .http
            .delete(url)
            .basic_auth(&self.username, Some(&self.password));
        if let Some(etag) = if_match {
            req = req.header("If-Match", etag);
        }
        let response = req.send().await.map_err(|e| format!("DELETE 失败: {e}"))?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        if status >= 400 && status != 404 && status != 412 {
            eprintln!("{}", format_http_err("DELETE", url, status, &text));
        }
        Ok((status, text))
    }

    pub async fn mkcalendar(
        &self,
        url: &str,
        display_name: &str,
        color: Option<&str>,
    ) -> Result<(), String> {
        let color_xml = match color.map(str::trim).filter(|c| is_calendar_color(c)) {
            Some(c) => format!(
                "\n      <ICAL:calendar-color>{}</ICAL:calendar-color>",
                xml_escape(c)
            ),
            None => String::new(),
        };
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<C:mkcalendar xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav" xmlns:ICAL="http://apple.com/ns/ical/">
  <D:set>
    <D:prop>
      <D:displayname>{}</D:displayname>{color_xml}
      <C:supported-calendar-component-set>
        <C:comp name="VEVENT"/>
        <C:comp name="VTODO"/>
        <C:comp name="VJOURNAL"/>
      </C:supported-calendar-component-set>
    </D:prop>
  </D:set>
</C:mkcalendar>"#,
            xml_escape(display_name)
        );
        let response = self
            .http
            .request(reqwest::Method::from_bytes(b"MKCALENDAR").unwrap(), url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body)
            .send()
            .await
            .map_err(|e| format!("MKCALENDAR 失败: {e}"))?;
        let status = response.status();
        if !status.is_success() && status.as_u16() != 201 {
            return Err(format!("MKCALENDAR {} 返回 {}", url, status.as_u16()));
        }
        Ok(())
    }
}

fn is_calendar_color(c: &str) -> bool {
    let rest = c.strip_prefix('#').unwrap_or("");
    matches!(rest.len(), 3 | 6 | 8) && rest.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn format_http_err(method: &str, url: &str, status: u16, body: &str) -> String {
    let detail = if body.trim().is_empty() {
        "(无响应体)".to_string()
    } else {
        truncate_err(body)
    };
    format!("{method} {url} 失败 {status}: {detail}")
}

fn preview_ics(ics: &str) -> String {
    let compact = ics.trim();
    if compact.is_empty() {
        return "(空)".into();
    }
    truncate_err(compact)
}

fn truncate_err(s: &str) -> String {
    const MAX: usize = 400;
    if s.chars().count() <= MAX {
        return s.to_string();
    }
    format!("{}…", s.chars().take(MAX).collect::<String>())
}
