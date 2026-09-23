use super::client::CaldavClient;
use super::multistatus::{parse_multistatus, prop_text};

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
    let mut out = Vec::new();
    for item in items {
        if item.props.is_calendar {
            continue;
        }
        let Some(ics) = prop_text(&item.props, &["calendar-data"]) else {
            continue;
        };
        if !ics.contains("BEGIN:") {
            continue;
        }
        out.push(CollectionObject {
            href: item.href,
            etag: prop_text(&item.props, &["getetag"]),
            ics,
        });
    }
    Ok(out)
}
