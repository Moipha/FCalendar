use chrono::NaiveDate;
use icalendar::{Calendar, Component, Event, EventLike, Property};

use super::time::{format_date, to_ics_date};

#[derive(Debug, Clone)]
pub struct EventDraft {
    pub uid: String,
    pub summary: String,
    pub description: Option<String>,
    pub all_day: bool,
    pub dtstart: String,
    pub dtend: String,
    pub rrule: Option<String>,
}

pub fn build_event_ics(draft: &EventDraft) -> Result<String, String> {
    if draft.summary.trim().is_empty() {
        return Err("summary is required".into());
    }

    let mut event = Event::new();
    event.uid(&draft.uid).summary(draft.summary.trim());

    if let Some(description) = draft
        .description
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        event.description(description);
    }

    if draft.all_day {
        let start = super::time::parse_date(&draft.dtstart)?;
        let end_inclusive = super::time::parse_date(&draft.dtend)?;
        if end_inclusive < start {
            return Err("all-day end must be on or after start".into());
        }
        let end_exclusive = end_inclusive
            .succ_opt()
            .ok_or_else(|| "all-day end date overflow".to_string())?;
        append_date_property(&mut event, "DTSTART", start);
        append_date_property(&mut event, "DTEND", end_exclusive);
    } else {
        let start = super::time::parse_offset_datetime(&draft.dtstart)?;
        let end = super::time::parse_offset_datetime(&draft.dtend)?;
        if end <= start {
            return Err("end must be after start".into());
        }
        event.starts(start.with_timezone(&chrono::Utc));
        event.ends(end.with_timezone(&chrono::Utc));
    }

    if let Some(rrule) = draft
        .rrule
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        event.add_property("RRULE", rrule);
    }

    let mut calendar = Calendar::new();
    calendar.push(event.done());
    Ok(calendar.to_string())
}

fn append_date_property(event: &mut Event, key: &str, date: NaiveDate) {
    event.append_property(
        Property::new(key, to_ics_date(date))
            .append_parameter(icalendar::Parameter::new("VALUE", "DATE"))
            .done(),
    );
}

pub fn draft_from_row(
    uid: &str,
    summary: &str,
    description: Option<&str>,
    all_day: bool,
    dtstart: &str,
    dtend: Option<&str>,
    rrule: Option<&str>,
) -> EventDraft {
    EventDraft {
        uid: uid.to_string(),
        summary: summary.to_string(),
        description: description.map(str::to_string),
        all_day,
        dtstart: dtstart.to_string(),
        dtend: dtend.unwrap_or(dtstart).to_string(),
        rrule: rrule.map(str::to_string),
    }
}

pub fn inclusive_all_day_end_from_exclusive(exclusive: NaiveDate) -> NaiveDate {
    exclusive.pred_opt().unwrap_or(exclusive)
}

pub fn exclusive_all_day_end_from_inclusive(inclusive: NaiveDate) -> NaiveDate {
    inclusive.succ_opt().unwrap_or(inclusive)
}

pub fn format_all_day_end_for_form(exclusive_or_same: &str, dtstart: &str) -> String {
    if let (Ok(start), Ok(end)) = (
        super::time::parse_date(dtstart),
        super::time::parse_date(exclusive_or_same),
    ) {
        if end > start {
            return format_date(inclusive_all_day_end_from_exclusive(end));
        }
    }
    dtstart.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_day_ics_uses_exclusive_dtend() {
        let ics = build_event_ics(&EventDraft {
            uid: "uid-1".into(),
            summary: "Holiday".into(),
            description: None,
            all_day: true,
            dtstart: "2026-09-17".into(),
            dtend: "2026-09-18".into(),
            rrule: None,
        })
        .unwrap();
        assert!(ics.contains("DTSTART;VALUE=DATE:20260917"));
        assert!(ics.contains("DTEND;VALUE=DATE:20260919"));
    }
}
