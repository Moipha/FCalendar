use chrono::{DateTime, Duration, FixedOffset, Offset};
use rrule::{RRuleSet, Tz};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::calendars::calendar_has_account;
use super::ics::{build_event_ics, EventDraft};
use super::time::{
    format_date, format_offset_datetime, parse_date, parse_offset_datetime, to_ics_date,
    to_ics_datetime, window_end_exclusive, window_start, now_millis,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRow {
    pub id: String,
    pub calendar_id: String,
    pub uid: String,
    pub summary: String,
    pub description: Option<String>,
    pub all_day: bool,
    pub dtstart: String,
    pub dtend: Option<String>,
    pub rrule: Option<String>,
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventInstance {
    pub instance_id: String,
    pub event_id: String,
    pub calendar_id: String,
    pub summary: String,
    pub description: Option<String>,
    pub all_day: bool,
    pub dtstart: String,
    pub dtend: String,
    pub rrule: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveEventInput {
    pub calendar_id: String,
    pub summary: String,
    pub description: Option<String>,
    pub all_day: bool,
    pub dtstart: String,
    pub dtend: String,
    pub rrule: Option<String>,
}

fn map_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRow> {
    Ok(EventRow {
        id: row.get(0)?,
        calendar_id: row.get(1)?,
        uid: row.get(2)?,
        summary: row.get(3)?,
        description: row.get(4)?,
        all_day: row.get::<_, i64>(5)? == 1,
        dtstart: row.get(6)?,
        dtend: row.get(7)?,
        rrule: row.get(8)?,
        dirty: row.get::<_, i64>(9)? == 1,
    })
}

fn load_master_events(conn: &Connection, calendar_id: Option<&str>) -> Result<Vec<EventRow>, String> {
    let sql = if calendar_id.is_some() {
        "SELECT id, calendar_id, uid, summary, description, all_day, dtstart, dtend, rrule, dirty
         FROM events
         WHERE deleted_at IS NULL AND calendar_id = ?1"
    } else {
        "SELECT id, calendar_id, uid, summary, description, all_day, dtstart, dtend, rrule, dirty
         FROM events
         WHERE deleted_at IS NULL"
    };

    let mut stmt = conn.prepare(sql).map_err(|e| format!("prepare events: {e}"))?;
    let rows = if let Some(calendar_id) = calendar_id {
        stmt.query_map(params![calendar_id], map_event_row)
    } else {
        stmt.query_map([], map_event_row)
    }
    .map_err(|e| format!("query events: {e}"))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| format!("collect events: {e}"))?;
    Ok(rows)
}

pub fn get_event(conn: &Connection, id: &str) -> Result<EventRow, String> {
    let mut row = conn
        .query_row(
            "SELECT id, calendar_id, uid, summary, description, all_day, dtstart, dtend, rrule, dirty
             FROM events
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            map_event_row,
        )
        .map_err(|e| format!("get event: {e}"))?;

    if row.all_day {
        if let Some(end_exclusive) = row.dtend.as_deref() {
            let end_exclusive = parse_date(end_exclusive)?;
            let end_inclusive = super::ics::inclusive_all_day_end_from_exclusive(end_exclusive);
            row.dtend = Some(format_date(end_inclusive));
        }
    }
    Ok(row)
}

pub fn list_event_instances(
    conn: &Connection,
    from: &str,
    to: &str,
    calendar_id: Option<&str>,
) -> Result<Vec<EventInstance>, String> {
    let window_start_date = parse_date(from)?;
    let window_end_date = parse_date(to)?;
    let range_start = window_start(window_start_date);
    let range_end = window_end_exclusive(window_end_date);

    let masters = load_master_events(conn, calendar_id)?;
    let mut instances = Vec::new();
    for master in masters {
        expand_master(&master, range_start, range_end, &mut instances)?;
    }
    instances.sort_by(|a, b| a.dtstart.cmp(&b.dtstart));
    Ok(instances)
}

fn expand_master(
    master: &EventRow,
    range_start: DateTime<FixedOffset>,
    range_end: DateTime<FixedOffset>,
    out: &mut Vec<EventInstance>,
) -> Result<(), String> {
    let has_rrule = master
        .rrule
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    if !has_rrule {
        if let Some(instance) = single_instance_if_in_window(master, range_start, range_end)? {
            out.push(instance);
        }
        return Ok(());
    }

    let rrule_body = master.rrule.as_ref().expect("rrule checked");
    let dtstart_line = if master.all_day {
        let start = parse_date(&master.dtstart)?;
        format!("DTSTART;VALUE=DATE:{}\nRRULE:{}", to_ics_date(start), rrule_body)
    } else {
        let start = parse_offset_datetime(&master.dtstart)?;
        format!(
            "DTSTART:{}\nRRULE:{}",
            to_ics_datetime(&start),
            rrule_body
        )
    };

    let set: RRuleSet = dtstart_line
        .parse()
        .map_err(|e| format!("parse rrule for {}: {e}", master.id))?;

    let local_start = range_start.with_timezone(&Tz::LOCAL);
    let local_end = range_end.with_timezone(&Tz::LOCAL);
    let result = set.after(local_start).before(local_end).all(500);

    if master.all_day {
        let duration_days = all_day_inclusive_days(master)?;
        for occurrence in result.dates {
            let occ_date = occurrence.date_naive();
            let end_inclusive = occ_date + Duration::days(duration_days as i64 - 1);
            out.push(build_instance(
                master,
                &format_date(occ_date),
                &format_date(end_inclusive),
            ));
        }
    } else {
        let duration = event_timed_duration(master)?;
        for occurrence in result.dates {
            let start: DateTime<FixedOffset> = occurrence.with_timezone(&local_offset());
            let end = start + duration;
            out.push(build_instance(
                master,
                &format_offset_datetime(&start),
                &format_offset_datetime(&end),
            ));
        }
    }

    Ok(())
}

fn single_instance_if_in_window(
    master: &EventRow,
    range_start: DateTime<FixedOffset>,
    range_end: DateTime<FixedOffset>,
) -> Result<Option<EventInstance>, String> {
    if master.all_day {
        let start = parse_date(&master.dtstart)?;
        let end_exclusive = master
            .dtend
            .as_deref()
            .map(parse_date)
            .transpose()?
            .unwrap_or_else(|| super::ics::exclusive_all_day_end_from_inclusive(start));
        let end_inclusive = super::ics::inclusive_all_day_end_from_exclusive(end_exclusive);
        let event_start = window_start(start);
        let event_end = window_end_exclusive(end_inclusive);
        if event_end <= range_start || event_start >= range_end {
            return Ok(None);
        }
        return Ok(Some(build_instance(
            master,
            &format_date(start),
            &format_date(end_inclusive),
        )));
    }

    let start = parse_offset_datetime(&master.dtstart)?;
    let end = master
        .dtend
        .as_deref()
        .map(parse_offset_datetime)
        .transpose()?
        .unwrap_or_else(|| start + Duration::hours(1));
    if end <= range_start || start >= range_end {
        return Ok(None);
    }
    Ok(Some(build_instance(
        master,
        &format_offset_datetime(&start),
        &format_offset_datetime(&end),
    )))
}

fn all_day_inclusive_days(master: &EventRow) -> Result<u32, String> {
    let start = parse_date(&master.dtstart)?;
    let end_exclusive = master
        .dtend
        .as_deref()
        .map(parse_date)
        .transpose()?
        .unwrap_or_else(|| super::ics::exclusive_all_day_end_from_inclusive(start));
    let end_inclusive = super::ics::inclusive_all_day_end_from_exclusive(end_exclusive);
    Ok((end_inclusive - start).num_days() as u32 + 1)
}

fn event_timed_duration(master: &EventRow) -> Result<Duration, String> {
    let start = parse_offset_datetime(&master.dtstart)?;
    let end = master
        .dtend
        .as_deref()
        .map(parse_offset_datetime)
        .transpose()?
        .unwrap_or_else(|| start + Duration::hours(1));
    Ok(end - start)
}

fn local_offset() -> FixedOffset {
    let seconds = chrono::Local::now().offset().fix().local_minus_utc();
    FixedOffset::east_opt(seconds).unwrap_or(FixedOffset::east_opt(0).unwrap())
}

fn build_instance(master: &EventRow, dtstart: &str, dtend: &str) -> EventInstance {
    let key = dtstart.replace([':', '+', '-', 'T'], "");
    EventInstance {
        instance_id: format!("{}_{}", master.id, key),
        event_id: master.id.clone(),
        calendar_id: master.calendar_id.clone(),
        summary: master.summary.clone(),
        description: master.description.clone(),
        all_day: master.all_day,
        dtstart: dtstart.to_string(),
        dtend: dtend.to_string(),
        rrule: master.rrule.clone(),
    }
}

pub fn create_event(conn: &Connection, input: SaveEventInput) -> Result<EventRow, String> {
    if input.summary.trim().is_empty() {
        return Err("summary is required".into());
    }

    let id = Uuid::new_v4().to_string();
    let uid = Uuid::new_v4().to_string();
    let now = now_millis();
    let dirty = if calendar_has_account(conn, &input.calendar_id)? {
        1
    } else {
        0
    };

    let (stored_start, stored_end) = normalize_stored_range(&input)?;
    let ics = build_event_ics(&EventDraft {
        uid: uid.clone(),
        summary: input.summary.clone(),
        description: input.description.clone(),
        all_day: input.all_day,
        dtstart: stored_start.clone(),
        dtend: stored_end.clone(),
        rrule: input.rrule.clone(),
    })?;

    conn.execute(
        "INSERT INTO events (
            id, calendar_id, uid, ics, summary, description, dtstart, dtend, all_day, rrule,
            dirty, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
        params![
            id,
            input.calendar_id,
            uid,
            ics,
            input.summary.trim(),
            input.description,
            stored_start,
            stored_end,
            i64::from(input.all_day),
            input.rrule,
            dirty,
            now,
        ],
    )
    .map_err(|e| format!("insert event: {e}"))?;

    get_event(conn, &id)
}

pub fn update_event(conn: &Connection, id: &str, input: SaveEventInput) -> Result<EventRow, String> {
    if input.summary.trim().is_empty() {
        return Err("summary is required".into());
    }

    let existing = get_event(conn, id)?;
    let now = now_millis();
    let dirty = if calendar_has_account(conn, &input.calendar_id)? {
        1
    } else {
        0
    };

    let (stored_start, stored_end) = normalize_stored_range(&input)?;
    let ics = build_event_ics(&EventDraft {
        uid: existing.uid.clone(),
        summary: input.summary.clone(),
        description: input.description.clone(),
        all_day: input.all_day,
        dtstart: stored_start.clone(),
        dtend: stored_end.clone(),
        rrule: input.rrule.clone(),
    })?;

    conn.execute(
        "UPDATE events SET
            calendar_id = ?2,
            ics = ?3,
            summary = ?4,
            description = ?5,
            dtstart = ?6,
            dtend = ?7,
            all_day = ?8,
            rrule = ?9,
            dirty = ?10,
            updated_at = ?11
         WHERE id = ?1 AND deleted_at IS NULL",
        params![
            id,
            input.calendar_id,
            ics,
            input.summary.trim(),
            input.description,
            stored_start,
            stored_end,
            i64::from(input.all_day),
            input.rrule,
            dirty,
            now,
        ],
    )
    .map_err(|e| format!("update event: {e}"))?;

    get_event(conn, &id)
}

pub fn delete_event(conn: &Connection, id: &str) -> Result<(), String> {
    let now = now_millis();
    let updated = conn
        .execute(
            "UPDATE events SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![id, now],
        )
        .map_err(|e| format!("delete event: {e}"))?;
    if updated == 0 {
        return Err(format!("event not found: {id}"));
    }
    Ok(())
}

fn normalize_stored_range(input: &SaveEventInput) -> Result<(String, String), String> {
    if input.all_day {
        let start = parse_date(&input.dtstart)?;
        let end_inclusive = parse_date(&input.dtend)?;
        if end_inclusive < start {
            return Err("all-day end must be on or after start".into());
        }
        let end_exclusive = super::ics::exclusive_all_day_end_from_inclusive(end_inclusive);
        Ok((format_date(start), format_date(end_exclusive)))
    } else {
        let start = parse_offset_datetime(&input.dtstart)?;
        let end = parse_offset_datetime(&input.dtend)?;
        if end <= start {
            return Err("end must be after start".into());
        }
        Ok((format_offset_datetime(&start), format_offset_datetime(&end)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::calendars::seed_default_calendar;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("sql/001_init.sql")).unwrap();
        seed_default_calendar(&conn).unwrap();
        conn
    }

    fn default_calendar_id(conn: &Connection) -> String {
        conn.query_row("SELECT id FROM calendars LIMIT 1", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn create_and_list_single_event() {
        let conn = test_conn();
        let calendar_id = default_calendar_id(&conn);
        create_event(
            &conn,
            SaveEventInput {
                calendar_id,
                summary: "Meeting".into(),
                description: None,
                all_day: false,
                dtstart: "2026-09-17T10:00:00+08:00".into(),
                dtend: "2026-09-17T11:00:00+08:00".into(),
                rrule: None,
            },
        )
        .unwrap();

        let instances = list_event_instances(&conn, "2026-09-01", "2026-09-30", None).unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].summary, "Meeting");
    }

    #[test]
    fn weekly_rrule_expands_in_window() {
        let conn = test_conn();
        let calendar_id = default_calendar_id(&conn);
        create_event(
            &conn,
            SaveEventInput {
                calendar_id,
                summary: "Weekly".into(),
                description: None,
                all_day: true,
                dtstart: "2026-09-03".into(),
                dtend: "2026-09-03".into(),
                rrule: Some("FREQ=WEEKLY".into()),
            },
        )
        .unwrap();

        let instances = list_event_instances(&conn, "2026-09-01", "2026-09-30", None).unwrap();
        assert!(instances.len() >= 4);
    }
}
