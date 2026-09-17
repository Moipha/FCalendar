use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, Offset, TimeZone, Utc};

pub fn now_millis() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| format!("invalid date '{s}': {e}"))
}

pub fn parse_offset_datetime(s: &str) -> Result<DateTime<FixedOffset>, String> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt);
    }
    if let Ok(dt) = DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%:z") {
        return Ok(dt);
    }
    if let Ok(dt) = DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z") {
        return Ok(dt);
    }

    let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
        .map_err(|e| format!("invalid datetime '{s}': {e}"))?;
    local_offset()
        .from_local_datetime(&naive)
        .single()
        .ok_or_else(|| format!("ambiguous local datetime '{s}'"))
}

pub fn format_offset_datetime(dt: &DateTime<FixedOffset>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

pub fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn to_ics_datetime(dt: &DateTime<FixedOffset>) -> String {
    dt.format("%Y%m%dT%H%M%S").to_string()
}

pub fn to_ics_date(date: NaiveDate) -> String {
    date.format("%Y%m%d").to_string()
}

pub fn window_start(date: NaiveDate) -> DateTime<FixedOffset> {
    let naive = date.and_hms_opt(0, 0, 0).expect("valid midnight");
    local_offset()
        .from_local_datetime(&naive)
        .single()
        .unwrap_or_else(|| local_offset().from_utc_datetime(&naive))
}

pub fn window_end_exclusive(date: NaiveDate) -> DateTime<FixedOffset> {
    let next = date.succ_opt().expect("valid next day");
    window_start(next)
}

fn local_offset() -> FixedOffset {
    let seconds = chrono::Local::now().offset().fix().local_minus_utc();
    FixedOffset::east_opt(seconds).unwrap_or(FixedOffset::east_opt(0).unwrap())
}

pub fn add_one_hour(start: &DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    *start + chrono::Duration::hours(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_offset_datetime_accepts_rfc3339() {
        let dt = parse_offset_datetime("2026-09-17T10:00:00+08:00").unwrap();
        assert_eq!(dt.offset().fix().local_minus_utc(), 8 * 3600);
    }
}
