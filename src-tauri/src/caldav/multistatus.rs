use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct PropValues {
    pub values: HashMap<String, String>,
    pub components: Vec<String>,
    pub is_calendar: bool,
}

#[derive(Debug, Clone)]
pub struct ResponseItem {
    pub href: String,
    pub props: PropValues,
}

/// 解析 WebDAV Multi-Status 响应（仅合并 propstat 200 的 prop）。
pub fn parse_multistatus(xml: &str) -> Result<Vec<ResponseItem>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut responses = Vec::new();
    let mut in_response = false;
    let mut in_propstat = false;
    let mut in_prop = false;
    let mut in_resourcetype = false;
    let mut propstat_status = String::new();
    let mut pending_props = PropValues::default();
    let mut current_href = String::new();
    let mut merged_props = PropValues::default();
    let mut active_prop: Option<String> = None;
    let mut nested_href_target: Option<String> = None;
    let mut text_buf = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Empty(e)) => {
                let local = e.local_name().as_ref().to_vec();
                let name = xml_local_name(&local);
                if name == "calendar" && in_resourcetype {
                    pending_props.is_calendar = true;
                } else if name == "comp" && in_prop {
                    apply_comp_attrs(&e, &mut pending_props);
                }
            }
            Ok(Event::Start(e)) => {
                let local = e.local_name().as_ref().to_vec();
                let name = xml_local_name(&local);
                match name {
                    "response" => {
                        in_response = true;
                        current_href.clear();
                        merged_props = PropValues::default();
                    }
                    "propstat" if in_response => {
                        in_propstat = true;
                        propstat_status.clear();
                        pending_props = PropValues::default();
                    }
                    "prop" if in_propstat => in_prop = true,
                    "resourcetype" if in_prop => in_resourcetype = true,
                    "calendar" if in_resourcetype => pending_props.is_calendar = true,
                    "comp" if in_prop => apply_comp_attrs(&e, &mut pending_props),
                    "href" if in_prop && active_prop.is_some() => {
                        nested_href_target = active_prop.clone();
                    }
                    _ if in_prop && !in_resourcetype => {
                        active_prop = Some(name.to_string());
                        text_buf.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                text_buf.push_str(&t.unescape().unwrap_or_default());
            }
            Ok(Event::CData(t)) => {
                text_buf.push_str(&String::from_utf8_lossy(t.as_ref()));
            }
            Ok(Event::End(e)) => {
                let local = e.local_name().as_ref().to_vec();
                let name = xml_local_name(&local);
                match name {
                    "response" => {
                        if !current_href.is_empty() {
                            responses.push(ResponseItem {
                                href: current_href.clone(),
                                props: merged_props.clone(),
                            });
                        }
                        in_response = false;
                    }
                    "propstat" => {
                        if propstat_status.contains("200") {
                            merge_props(&mut merged_props, &pending_props);
                        }
                        in_propstat = false;
                        in_prop = false;
                        in_resourcetype = false;
                        active_prop = None;
                        pending_props = PropValues::default();
                    }
                    "prop" => {
                        in_prop = false;
                        in_resourcetype = false;
                        active_prop = None;
                    }
                    "resourcetype" => in_resourcetype = false,
                    "href" if in_response && nested_href_target.is_none() => {
                        current_href = normalize_href(&text_buf);
                        text_buf.clear();
                    }
                    "href" if in_prop && nested_href_target.is_some() => {
                        if let Some(key) = nested_href_target.take() {
                            pending_props
                                .values
                                .insert(key, normalize_href(&text_buf));
                        }
                        text_buf.clear();
                    }
                    "status" if in_propstat => {
                        propstat_status = text_buf.clone();
                        text_buf.clear();
                    }
                    prop_name if in_prop && in_propstat => {
                        if let Some(key) = active_prop.take() {
                            if key == prop_name && !text_buf.is_empty() {
                                pending_props.values.insert(key, text_buf.clone());
                            }
                        }
                        text_buf.clear();
                    }
                    _ => {
                        text_buf.clear();
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("解析 XML: {e}")),
            _ => {}
        }
    }

    Ok(responses)
}

fn apply_comp_attrs(e: &quick_xml::events::BytesStart, props: &mut PropValues) {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"name" {
            let comp = String::from_utf8_lossy(&attr.value).into_owned();
            if comp.eq_ignore_ascii_case("VEVENT")
                || comp.eq_ignore_ascii_case("VTODO")
                || comp.eq_ignore_ascii_case("VJOURNAL")
            {
                props.is_calendar = true;
            }
            props.components.push(comp);
        }
    }
}

fn xml_local_name(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap_or("")
}

fn merge_props(into: &mut PropValues, from: &PropValues) {
    for (k, v) in &from.values {
        into.values.insert(k.clone(), v.clone());
    }
    if !from.components.is_empty() {
        into.components.clone_from(&from.components);
    }
    if from.is_calendar {
        into.is_calendar = true;
    }
}

fn normalize_href(href: &str) -> String {
    let trimmed = href.trim();
    if trimmed.ends_with('/') && trimmed.len() > 1 {
        trimmed.trim_end_matches('/').to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn prop_text(props: &PropValues, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(v) = props.values.get(*key) {
            if !v.is_empty() {
                return Some(v.clone());
            }
        }
    }
    None
}

pub fn supports_component(props: &PropValues, name: &str) -> bool {
    if props.components.is_empty() && props.is_calendar {
        return name.eq_ignore_ascii_case("VEVENT");
    }
    props
        .components
        .iter()
        .any(|c| c.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_calendar_collection() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav" xmlns:ICAL="http://apple.com/ns/ical/">
  <D:response>
    <D:href>/alice/work/</D:href>
    <D:propstat>
      <D:prop>
        <D:displayname>Work</D:displayname>
        <ICAL:calendar-color>#ff0000ff</ICAL:calendar-color>
        <D:resourcetype>
          <D:collection/>
          <C:calendar/>
        </D:resourcetype>
        <C:supported-calendar-component-set>
          <C:comp name="VEVENT"/>
          <C:comp name="VTODO"/>
        </C:supported-calendar-component-set>
        <C:getctag>ctag-1</C:getctag>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
</D:multistatus>"#;
        let items = parse_multistatus(xml).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].href, "/alice/work");
        assert!(items[0].props.is_calendar);
        assert_eq!(
            prop_text(&items[0].props, &["displayname"]),
            Some("Work".into())
        );
        assert!(supports_component(&items[0].props, "VEVENT"));
        assert!(supports_component(&items[0].props, "VTODO"));
    }

    #[test]
    fn calendar_data_keeps_multiline_ics() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/alice/work/evt.ics</D:href>
    <D:propstat>
      <D:prop>
        <D:getetag>"abc"</D:getetag>
        <C:calendar-data>BEGIN:VCALENDAR
BEGIN:VEVENT
UID:evt-1
SUMMARY:Hello
DTSTART;VALUE=DATE:20260923
END:VEVENT
END:VCALENDAR
</C:calendar-data>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
</D:multistatus>"#;
        let items = parse_multistatus(xml).unwrap();
        let ics = prop_text(&items[0].props, &["calendar-data"]).unwrap();
        assert!(ics.contains("BEGIN:VCALENDAR"));
        assert!(ics.contains("UID:evt-1"));
        assert!(ics.contains("END:VCALENDAR"));
    }
}
