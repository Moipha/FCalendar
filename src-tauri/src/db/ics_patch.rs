use super::ics::{build_event_ics, EventDraft};
use super::time::{parse_date, parse_offset_datetime, to_ics_date};

const X_STAMP: &str = "X-FC-IS-STAMP";
const X_SORT: &str = "X-FC-SORT-ORDER";
const X_DAY_COLOR: &str = "X-FC-DAY-COLOR";

#[derive(Debug, Clone)]
pub struct TaskDraft {
    pub uid: String,
    pub summary: String,
    pub description: Option<String>,
    pub is_stamp: bool,
    pub sort_order: i64,
}

#[derive(Debug, Clone)]
pub struct DayColorDraft {
    pub date: String,
    pub color: String,
}

#[derive(Debug, Clone)]
pub enum RemoteObject {
    Event {
        uid: String,
        summary: String,
        description: Option<String>,
        all_day: bool,
        dtstart: String,
        dtend: Option<String>,
        rrule: Option<String>,
        ics: String,
    },
    Task {
        uid: String,
        summary: String,
        description: Option<String>,
        is_stamp: bool,
        sort_order: i64,
        ics: String,
    },
    DayColor {
        date: String,
        color: String,
        ics: String,
    },
}

pub fn event_ics_from_existing(existing: Option<&str>, draft: &EventDraft) -> Result<String, String> {
    if let Some(raw) = existing.filter(|s| !s.trim().is_empty()) {
        if let Some(patched) = upsert_component(raw, "VEVENT", &event_known_lines(draft)?) {
            return Ok(patched);
        }
    }
    build_event_ics(draft)
}

pub fn task_ics_from_existing(existing: Option<&str>, draft: &TaskDraft) -> Result<String, String> {
    if let Some(raw) = existing.filter(|s| !s.trim().is_empty()) {
        if let Some(patched) = upsert_component(raw, "VTODO", &task_known_lines(draft)) {
            return Ok(patched);
        }
    }
    wrap_component("VTODO", &task_known_lines(draft))
}

pub fn day_color_ics_from_existing(
    existing: Option<&str>,
    draft: &DayColorDraft,
) -> Result<String, String> {
    if let Some(raw) = existing.filter(|s| !s.trim().is_empty()) {
        if let Some(patched) = upsert_component(raw, "VJOURNAL", &day_color_known_lines(draft)?) {
            return Ok(patched);
        }
    }
    wrap_component("VJOURNAL", &day_color_known_lines(draft)?)
}

pub fn day_color_uid(date: &str) -> String {
    format!("fc-daycolor-{date}")
}

fn unfold_ics(ics: &str) -> String {
    let mut out = String::new();
    for line in ics.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            out.push_str(line.trim_start());
        } else {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line.trim_end());
        }
    }
    out
}

pub fn classify_ics(ics: &str) -> Option<RemoteObject> {
    let ics = unfold_ics(ics);
    let block = first_component(&ics)?;
    let name = block.name.as_str();
    match name {
        "VEVENT" => {
            let dtstart = parse_ics_dt(&prop_line(&block.lines, "DTSTART")?)?;
            Some(RemoteObject::Event {
                uid: prop(&block.lines, "UID").unwrap_or_default(),
                summary: prop(&block.lines, "SUMMARY").unwrap_or_default(),
                description: prop(&block.lines, "DESCRIPTION"),
                all_day: !dtstart.contains('T'),
                dtstart,
                dtend: prop_line(&block.lines, "DTEND").and_then(|l| parse_ics_dt(&l)),
                rrule: prop(&block.lines, "RRULE"),
                ics,
            })
        }
        "VTODO" => {
            if prop_line(&block.lines, "DTSTART").is_some() {
                return None;
            }
            let is_stamp = prop(&block.lines, X_STAMP)
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            Some(RemoteObject::Task {
                uid: prop(&block.lines, "UID").unwrap_or_default(),
                summary: prop(&block.lines, "SUMMARY").unwrap_or_default(),
                description: prop(&block.lines, "DESCRIPTION"),
                is_stamp,
                sort_order: prop(&block.lines, X_SORT)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0),
                ics,
            })
        }
        "VJOURNAL" => {
            let color = prop(&block.lines, X_DAY_COLOR)?.to_ascii_lowercase();
            if color != "green" && color != "red" {
                return None;
            }
            let uid = prop(&block.lines, "UID").unwrap_or_default();
            let date = uid
                .strip_prefix("fc-daycolor-")
                .map(|s| s.to_string())
                .or_else(|| {
                    prop_line(&block.lines, "DTSTART").and_then(|l| parse_ics_dt(&l))
                })?;
            Some(RemoteObject::DayColor {
                date,
                color,
                ics,
            })
        }
        _ => None,
    }
}

struct CompBlock {
    name: String,
    lines: Vec<String>,
}

fn first_component(ics: &str) -> Option<CompBlock> {
    let mut name = None;
    let mut lines = Vec::new();
    let mut inner = false;
    for raw in ics.lines() {
        let line = raw.trim_end();
        if let Some(rest) = line.strip_prefix("BEGIN:") {
            let n = rest.trim();
            if n != "VCALENDAR" && name.is_none() {
                name = Some(n.to_string());
                inner = true;
                continue;
            }
        }
        if inner {
            if line.starts_with("END:") {
                break;
            }
            lines.push(line.to_string());
        }
    }
    Some(CompBlock { name: name?, lines })
}

fn prop(lines: &[String], key: &str) -> Option<String> {
    prop_line(lines, key).map(|l| {
        l.split_once(':')
            .map(|(_, v)| v.to_string())
            .unwrap_or_default()
    })
}

fn prop_line(lines: &[String], key: &str) -> Option<String> {
    let prefix = format!("{key}");
    lines.iter().find(|l| {
        l.starts_with(&format!("{prefix}:")) || l.starts_with(&format!("{prefix};"))
    }).cloned()
}

fn parse_ics_dt(line: &str) -> Option<String> {
    let value = line.split_once(':')?.1;
    if line.contains("VALUE=DATE") || value.len() == 8 {
        let y = value.get(0..4)?;
        let m = value.get(4..6)?;
        let d = value.get(6..8)?;
        return Some(format!("{y}-{m}-{d}"));
    }
    let compact = value.trim_end_matches('Z');
    if compact.len() >= 15 {
        let y = compact.get(0..4)?;
        let m = compact.get(4..6)?;
        let d = compact.get(6..8)?;
        let hh = compact.get(9..11)?;
        let mm = compact.get(11..13)?;
        let ss = compact.get(13..15)?;
        if value.ends_with('Z') {
            return Some(format!("{y}-{m}-{d}T{hh}:{mm}:{ss}+00:00"));
        }
        return Some(format!("{y}-{m}-{d}T{hh}:{mm}:{ss}+00:00"));
    }
    None
}

fn utc_stamp() -> String {
    chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}

fn escape_ics_text(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\r', "")
        .replace('\n', "\\n")
}

fn event_known_lines(draft: &EventDraft) -> Result<Vec<String>, String> {
    let mut lines = vec![
        format!("UID:{}", draft.uid),
        format!("DTSTAMP:{}", utc_stamp()),
        format!("SUMMARY:{}", escape_ics_text(draft.summary.trim())),
    ];
    if let Some(d) = draft
        .description
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        lines.push(format!("DESCRIPTION:{}", escape_ics_text(d)));
    }
    if draft.all_day {
        let start = parse_date(&draft.dtstart)?;
        let end_inclusive = parse_date(&draft.dtend)?;
        let end_exclusive = end_inclusive
            .succ_opt()
            .ok_or_else(|| "all-day end date overflow".to_string())?;
        lines.push(format!("DTSTART;VALUE=DATE:{}", to_ics_date(start)));
        lines.push(format!("DTEND;VALUE=DATE:{}", to_ics_date(end_exclusive)));
    } else {
        let start = parse_offset_datetime(&draft.dtstart)?;
        let end = parse_offset_datetime(&draft.dtend)?;
        lines.push(format!(
            "DTSTART:{}Z",
            start.with_timezone(&chrono::Utc).format("%Y%m%dT%H%M%S")
        ));
        lines.push(format!(
            "DTEND:{}Z",
            end.with_timezone(&chrono::Utc).format("%Y%m%dT%H%M%S")
        ));
    }
    if let Some(rrule) = draft
        .rrule
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        lines.push(format!("RRULE:{rrule}"));
    }
    Ok(lines)
}

fn task_known_lines(draft: &TaskDraft) -> Vec<String> {
    let mut lines = vec![
        format!("UID:{}", draft.uid),
        format!("DTSTAMP:{}", utc_stamp()),
        format!("SUMMARY:{}", escape_ics_text(draft.summary.trim())),
        format!("{X_SORT}:{}", draft.sort_order),
    ];
    if let Some(d) = draft
        .description
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        lines.push(format!("DESCRIPTION:{}", escape_ics_text(d)));
    }
    if draft.is_stamp {
        lines.push(format!("{X_STAMP}:1"));
    }
    lines
}

fn day_color_known_lines(draft: &DayColorDraft) -> Result<Vec<String>, String> {
    let date = parse_date(&draft.date)?;
    Ok(vec![
        format!("UID:{}", day_color_uid(&draft.date)),
        format!("DTSTAMP:{}", utc_stamp()),
        format!("DTSTART;VALUE=DATE:{}", to_ics_date(date)),
        format!("{X_DAY_COLOR}:{}", draft.color),
    ])
}

fn wrap_component(name: &str, known: &[String]) -> Result<String, String> {
    let mut out = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//FCalendar//EN\r\n");
    out.push_str(&format!("BEGIN:{name}\r\n"));
    for line in known {
        out.push_str(line);
        out.push_str("\r\n");
    }
    out.push_str(&format!("END:{name}\r\nEND:VCALENDAR\r\n"));
    Ok(out)
}

fn upsert_component(ics: &str, name: &str, known: &[String]) -> Option<String> {
    let begin = format!("BEGIN:{name}");
    let end = format!("END:{name}");
    let start = ics.find(&begin)?;
    let end_idx = ics[start..].find(&end)? + start;
    let inner = &ics[start + begin.len()..end_idx];
    let mut kept = Vec::new();
    for line in inner.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let key = line.split([':', ';']).next().unwrap_or("");
        if known.iter().any(|k| k.split([':', ';']).next() == Some(key)) {
            continue;
        }
        kept.push(line.to_string());
    }
    let mut rebuilt = String::new();
    rebuilt.push_str(&begin);
    rebuilt.push('\n');
    for line in known {
        rebuilt.push_str(line);
        rebuilt.push('\n');
    }
    for line in kept {
        rebuilt.push_str(&line);
        rebuilt.push('\n');
    }
    rebuilt.push_str(&end);
    Some(format!("{}{}{}", &ics[..start], rebuilt, &ics[end_idx + end.len()..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_event_keeps_unknown_property() {
        let original = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:uid-1\nSUMMARY:Old\nDTSTART;VALUE=DATE:20260917\nDTEND;VALUE=DATE:20260918\nX-FOO:bar\nEND:VEVENT\nEND:VCALENDAR\n";
        let ics = event_ics_from_existing(
            Some(original),
            &EventDraft {
                uid: "uid-1".into(),
                summary: "New".into(),
                description: None,
                all_day: true,
                dtstart: "2026-09-17".into(),
                dtend: "2026-09-17".into(),
                rrule: None,
            },
        )
        .unwrap();
        assert!(ics.contains("SUMMARY:New"));
        assert!(ics.contains("X-FOO:bar"));
        assert!(ics.contains("DTSTAMP:"));
    }

    #[test]
    fn classify_generated_event_ics() {
        let ics = crate::db::ics::build_event_ics(&EventDraft {
            uid: "uid-gen".into(),
            summary: "会议".into(),
            description: None,
            all_day: false,
            dtstart: "2026-09-23T10:00:00+08:00".into(),
            dtend: "2026-09-23T11:00:00+08:00".into(),
            rrule: None,
        })
        .unwrap();
        match classify_ics(&ics) {
            Some(RemoteObject::Event { uid, summary, .. }) => {
                assert_eq!(uid, "uid-gen");
                assert_eq!(summary, "会议");
            }
            other => panic!("expected event, got {other:?}\n{ics}"),
        }
    }

    #[test]
    fn classify_date_only_dtstart_is_all_day_even_without_value_param() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:d1\nSUMMARY:Day\nDTSTART:20260925\nDTEND:20260926\nEND:VEVENT\nEND:VCALENDAR\n";
        match classify_ics(ics) {
            Some(RemoteObject::Event {
                all_day,
                dtstart,
                dtend,
                ..
            }) => {
                assert!(all_day);
                assert_eq!(dtstart, "2026-09-25");
                assert_eq!(dtend.as_deref(), Some("2026-09-26"));
            }
            other => panic!("expected all-day event, got {other:?}"),
        }
    }

    #[test]
    fn task_stamp_roundtrip() {
        let ics = task_ics_from_existing(
            None,
            &TaskDraft {
                uid: "t1".into(),
                summary: "Home".into(),
                description: None,
                is_stamp: true,
                sort_order: -3,
            },
        )
        .unwrap();
        assert!(ics.contains("DTSTAMP:"));
        match classify_ics(&ics) {
            Some(RemoteObject::Task {
                is_stamp, sort_order, ..
            }) => {
                assert!(is_stamp);
                assert_eq!(sort_order, -3);
            }
            other => panic!("expected task, got {other:?}"),
        }
    }
}
