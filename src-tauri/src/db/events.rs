use chrono::{DateTime, Duration, FixedOffset, Offset};
use rrule::{RRuleSet, Tz};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::calendars::calendar_has_account;
use super::ics::EventDraft;
use super::ics_patch::event_ics_from_existing;
use super::queue;
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
            row.dtend = Some(super::ics::format_all_day_end_for_form(
                end_exclusive,
                &row.dtstart,
            ));
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
    let ics = event_ics_from_existing(
        None,
        &draft_from_save(&uid, &input, &stored_start, &stored_end),
    )?;

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

    if dirty == 1 {
        queue::enqueue(conn, "event", &id, "create")?;
    }
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

    let existing_ics: String = conn
        .query_row(
            "SELECT ics FROM events WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| format!("load event ics: {e}"))?;
    let (stored_start, stored_end) = normalize_stored_range(&input)?;
    let ics = event_ics_from_existing(
        Some(&existing_ics),
        &draft_from_save(&existing.uid, &input, &stored_start, &stored_end),
    )?;

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

    if dirty == 1 {
        queue::enqueue(conn, "event", id, "update")?;
    }
    get_event(conn, &id)
}

pub fn delete_event(conn: &Connection, id: &str) -> Result<(), String> {
    let now = now_millis();
    let calendar_id: String = conn
        .query_row(
            "SELECT calendar_id FROM events WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| format!("event not found: {e}"))?;
    let remote = calendar_has_account(conn, &calendar_id)?;
    let updated = conn
        .execute(
            "UPDATE events SET deleted_at = ?2, updated_at = ?2, dirty = ?3 WHERE id = ?1 AND deleted_at IS NULL",
            params![id, now, i64::from(remote)],
        )
        .map_err(|e| format!("delete event: {e}"))?;
    if updated == 0 {
        return Err(format!("event not found: {id}"));
    }
    if remote {
        queue::enqueue(conn, "event", id, "delete")?;
    }
    Ok(())
}

pub struct EventSyncRow {
    pub id: String,
    pub uid: String,
    pub href: Option<String>,
    pub etag: Option<String>,
    pub ics: String,
    pub dirty: bool,
    pub deleted_at: Option<i64>,
    pub summary: String,
    pub description: Option<String>,
    pub dtstart: String,
    pub dtend: Option<String>,
    pub all_day: bool,
    pub rrule: Option<String>,
}

pub fn list_sync_events(conn: &Connection, calendar_id: &str) -> Result<Vec<EventSyncRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, uid, href, etag, ics, dirty, deleted_at,
                    summary, description, dtstart, dtend, all_day, rrule
             FROM events WHERE calendar_id = ?1",
        )
        .map_err(|e| format!("prepare sync events: {e}"))?;
    let rows = stmt
        .query_map(params![calendar_id], |row| {
            Ok(EventSyncRow {
                id: row.get(0)?,
                uid: row.get(1)?,
                href: row.get(2)?,
                etag: row.get(3)?,
                ics: row.get(4)?,
                dirty: row.get::<_, i64>(5)? == 1,
                deleted_at: row.get(6)?,
                summary: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                description: row.get(8)?,
                dtstart: row.get(9)?,
                dtend: row.get(10)?,
                all_day: row.get::<_, i64>(11)? == 1,
                rrule: row.get(12)?,
            })
        })
        .map_err(|e| format!("query sync events: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect sync events: {e}"))?;
    Ok(rows)
}

pub fn upsert_remote_event(
    conn: &Connection,
    calendar_id: &str,
    uid: &str,
    href: &str,
    etag: Option<&str>,
    ics: &str,
    summary: &str,
    description: Option<&str>,
    all_day: bool,
    dtstart: &str,
    dtend: Option<&str>,
    rrule: Option<&str>,
) -> Result<(), String> {
    let now = now_millis();
    let existing: Option<String> = match conn.query_row(
        "SELECT id FROM events WHERE calendar_id = ?1 AND uid = ?2",
        params![calendar_id, uid],
        |row| row.get(0),
    ) {
        Ok(id) => Some(id),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => return Err(format!("lookup event uid: {e}")),
    };
    if let Some(id) = existing {
        conn.execute(
            "UPDATE events SET href=?2, etag=?3, ics=?4, summary=?5, description=?6,
                all_day=?7, dtstart=?8, dtend=?9, rrule=?10, dirty=0, deleted_at=NULL, updated_at=?11
             WHERE id=?1",
            params![
                id,
                href,
                etag,
                ics,
                summary,
                description,
                i64::from(all_day),
                dtstart,
                dtend,
                rrule,
                now
            ],
        )
        .map_err(|e| format!("update remote event: {e}"))?;
    } else {
        conn.execute(
            "INSERT INTO events (
                id, calendar_id, uid, href, etag, ics, summary, description,
                dtstart, dtend, all_day, rrule, dirty, created_at, updated_at
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,0,?13,?13)",
            params![
                Uuid::new_v4().to_string(),
                calendar_id,
                uid,
                href,
                etag,
                ics,
                summary,
                description,
                dtstart,
                dtend,
                i64::from(all_day),
                rrule,
                now
            ],
        )
        .map_err(|e| format!("insert remote event: {e}"))?;
    }
    Ok(())
}

pub fn mark_event_synced(
    conn: &Connection,
    id: &str,
    href: &str,
    etag: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE events SET href=?2, etag=?3, dirty=0 WHERE id=?1",
        params![id, href, etag],
    )
    .map_err(|e| format!("mark event synced: {e}"))?;
    queue::drop_entity_queue(conn, "event", id)
}

pub fn hard_delete_event(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM events WHERE id = ?1", params![id])
        .map_err(|e| format!("hard delete event: {e}"))?;
    queue::drop_entity_queue(conn, "event", id)
}

/// EventDraft 的全天结束日是含当日；库列存 RFC 开区间。
fn draft_from_save(
    uid: &str,
    input: &SaveEventInput,
    stored_start: &str,
    stored_end: &str,
) -> EventDraft {
    EventDraft {
        uid: uid.to_string(),
        summary: input.summary.clone(),
        description: input.description.clone(),
        all_day: input.all_day,
        dtstart: if input.all_day {
            input.dtstart.clone()
        } else {
            stored_start.to_string()
        },
        dtend: if input.all_day {
            input.dtend.clone()
        } else {
            stored_end.to_string()
        },
        rrule: input.rrule.clone(),
    }
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

    #[test]
    fn all_day_create_ics_dtend_is_one_day_exclusive() {
        let conn = test_conn();
        let calendar_id = default_calendar_id(&conn);
        let row = create_event(
            &conn,
            SaveEventInput {
                calendar_id,
                summary: "Holiday".into(),
                description: None,
                all_day: true,
                dtstart: "2026-09-25".into(),
                dtend: "2026-09-25".into(),
                rrule: None,
            },
        )
        .unwrap();

        assert_eq!(row.dtstart, "2026-09-25");
        assert_eq!(row.dtend.as_deref(), Some("2026-09-25"));

        let ics: String = conn
            .query_row("SELECT ics FROM events WHERE id = ?1", params![row.id], |r| r.get(0))
            .unwrap();
        assert!(ics.contains("DTSTART;VALUE=DATE:20260925"), "{ics}");
        assert!(ics.contains("DTEND;VALUE=DATE:20260926"), "{ics}");
        assert!(!ics.contains("DTEND;VALUE=DATE:20260927"), "{ics}");

        let instances = list_event_instances(&conn, "2026-09-01", "2026-09-30", None).unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].dtstart, "2026-09-25");
        assert_eq!(instances[0].dtend, "2026-09-25");
    }

    #[test]
    fn all_day_remote_upsert_form_end_stays_inclusive() {
        use crate::db::ics_patch::{classify_ics, RemoteObject};

        let conn = test_conn();
        let calendar_id = default_calendar_id(&conn);
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:pull-1\nSUMMARY:Holiday\nDTSTART;VALUE=DATE:20260925\nDTEND;VALUE=DATE:20260926\nEND:VEVENT\nEND:VCALENDAR\n";
        let RemoteObject::Event {
            uid,
            summary,
            description,
            all_day,
            dtstart,
            dtend,
            rrule,
            ics,
        } = classify_ics(ics).expect("event")
        else {
            panic!("expected event");
        };
        assert!(all_day);
        upsert_remote_event(
            &conn,
            &calendar_id,
            &uid,
            "/cal/pull-1.ics",
            Some("etag"),
            &ics,
            &summary,
            description.as_deref(),
            all_day,
            &dtstart,
            dtend.as_deref(),
            rrule.as_deref(),
        )
        .unwrap();
        let id: String = conn
            .query_row(
                "SELECT id FROM events WHERE uid = ?1",
                params!["pull-1"],
                |row| row.get(0),
            )
            .unwrap();
        let row = get_event(&conn, &id).unwrap();
        assert_eq!(row.dtstart, "2026-09-25");
        assert_eq!(row.dtend.as_deref(), Some("2026-09-25"));
    }
}
