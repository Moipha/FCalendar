use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::app_state;
use super::calendars::calendar_has_account;
use super::ics_patch::{day_color_ics_from_existing, day_color_uid, DayColorDraft};
use super::queue;
use super::time::now_millis;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DayColorPreset {
    Green,
    Red,
}

impl DayColorPreset {
    fn as_str(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Red => "red",
        }
    }

    fn from_db(s: &str) -> Option<Self> {
        match s {
            "green" => Some(Self::Green),
            "red" => Some(Self::Red),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayColorRow {
    pub date: String,
    pub color: String,
}

fn calendar_or_current(conn: &Connection, calendar_id: Option<&str>) -> Result<String, String> {
    match calendar_id {
        Some(id) if !id.is_empty() => Ok(id.to_string()),
        _ => app_state::current_calendar_id(conn),
    }
}

pub fn list_day_colors(
    conn: &Connection,
    from: &str,
    to: &str,
    calendar_id: Option<&str>,
) -> Result<Vec<DayColorRow>, String> {
    let cal = calendar_or_current(conn, calendar_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT date, color FROM day_colors
             WHERE calendar_id = ?1 AND date >= ?2 AND date <= ?3 AND deleted_at IS NULL
             ORDER BY date ASC",
        )
        .map_err(|e| format!("prepare list_day_colors: {e}"))?;

    let rows = stmt
        .query_map(params![cal, from, to], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("query list_day_colors: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect list_day_colors: {e}"))?;

    Ok(rows
        .into_iter()
        .filter(|(_, c)| DayColorPreset::from_db(c).is_some())
        .map(|(date, color)| DayColorRow { date, color })
        .collect())
}

pub fn set_day_color(
    conn: &Connection,
    date: &str,
    color: Option<&str>,
    calendar_id: Option<&str>,
) -> Result<(), String> {
    if date.len() != 10 || !date.as_bytes().get(4).is_some_and(|b| *b == b'-') {
        return Err("invalid date".into());
    }
    let cal = calendar_or_current(conn, calendar_id)?;
    let remote = calendar_has_account(conn, &cal)?;
    let entity_id = format!("{cal}:{date}");

    match color.filter(|s| !s.is_empty()) {
        None => {
            if remote {
                let now = now_millis();
                conn.execute(
                    "UPDATE day_colors SET deleted_at=?3, dirty=1 WHERE calendar_id=?1 AND date=?2",
                    params![cal, date, now],
                )
                .map_err(|e| format!("tombstone day_color: {e}"))?;
                queue::enqueue(conn, "day_color", &entity_id, "delete")?;
            } else {
                conn.execute(
                    "DELETE FROM day_colors WHERE calendar_id = ?1 AND date = ?2",
                    params![cal, date],
                )
                .map_err(|e| format!("delete day_color: {e}"))?;
            }
        }
        Some(preset @ ("green" | "red")) => {
            let uid = day_color_uid(date);
            let existing_ics: Option<String> = conn
                .query_row(
                    "SELECT ics FROM day_colors WHERE calendar_id = ?1 AND date = ?2",
                    params![cal, date],
                    |row| row.get(0),
                )
                .ok()
                .flatten();
            let ics = day_color_ics_from_existing(
                existing_ics.as_deref(),
                &DayColorDraft {
                    date: date.to_string(),
                    color: preset.to_string(),
                },
            )?;
            conn.execute(
                "INSERT INTO day_colors (calendar_id, date, color, uid, ics, dirty, deleted_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)
                 ON CONFLICT(calendar_id, date) DO UPDATE SET
                    color=excluded.color, uid=excluded.uid, ics=excluded.ics,
                    dirty=excluded.dirty, deleted_at=NULL",
                params![cal, date, preset, uid, ics, i64::from(remote)],
            )
            .map_err(|e| format!("set day_color: {e}"))?;
            if remote {
                queue::enqueue(conn, "day_color", &entity_id, "update")?;
            }
            let _ = DayColorPreset::as_str;
        }
        Some(_) => return Err("invalid color".into()),
    }
    Ok(())
}

pub fn set_day_colors(
    conn: &Connection,
    dates: &[String],
    color: Option<&str>,
    calendar_id: Option<&str>,
) -> Result<(), String> {
    for date in dates {
        set_day_color(conn, date, color, calendar_id)?;
    }
    Ok(())
}

pub struct ColorSyncRow {
    pub date: String,
    pub uid: String,
    pub href: Option<String>,
    pub etag: Option<String>,
    pub ics: String,
    pub dirty: bool,
    pub deleted_at: Option<i64>,
}

pub fn list_sync_colors(conn: &Connection, calendar_id: &str) -> Result<Vec<ColorSyncRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT date, uid, href, etag, ics, dirty, deleted_at FROM day_colors WHERE calendar_id = ?1",
        )
        .map_err(|e| format!("prepare sync colors: {e}"))?;
    let rows = stmt
        .query_map(params![calendar_id], |row| {
            Ok(ColorSyncRow {
                date: row.get(0)?,
                uid: row.get(1)?,
                href: row.get(2)?,
                etag: row.get(3)?,
                ics: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                dirty: row.get::<_, i64>(5)? == 1,
                deleted_at: row.get(6)?,
            })
        })
        .map_err(|e| format!("query sync colors: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect sync colors: {e}"))?;
    Ok(rows)
}

pub fn upsert_remote_color(
    conn: &Connection,
    calendar_id: &str,
    date: &str,
    href: &str,
    etag: Option<&str>,
    ics: &str,
    color: &str,
) -> Result<(), String> {
    let uid = day_color_uid(date);
    conn.execute(
        "INSERT INTO day_colors (calendar_id, date, color, uid, href, etag, ics, dirty, deleted_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,0,NULL)
         ON CONFLICT(calendar_id, date) DO UPDATE SET
            color=excluded.color, uid=excluded.uid, href=excluded.href,
            etag=excluded.etag, ics=excluded.ics, dirty=0, deleted_at=NULL",
        params![calendar_id, date, color, uid, href, etag, ics],
    )
    .map_err(|e| format!("upsert color: {e}"))?;
    Ok(())
}

pub fn mark_color_synced(
    conn: &Connection,
    calendar_id: &str,
    date: &str,
    href: &str,
    etag: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE day_colors SET href=?3, etag=?4, dirty=0 WHERE calendar_id=?1 AND date=?2",
        params![calendar_id, date, href, etag],
    )
    .map_err(|e| format!("mark color synced: {e}"))?;
    queue::drop_entity_queue(conn, "day_color", &format!("{calendar_id}:{date}"))
}

pub fn hard_delete_color(conn: &Connection, calendar_id: &str, date: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM day_colors WHERE calendar_id = ?1 AND date = ?2",
        params![calendar_id, date],
    )
    .map_err(|e| format!("hard delete color: {e}"))?;
    queue::drop_entity_queue(conn, "day_color", &format!("{calendar_id}:{date}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::calendars::seed_default_calendar;
    use crate::db::migrate;

    fn test_conn() -> Connection {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!("fcalendar-day-colors-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("test-{}.db", COUNTER.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_file(&path);
        migrate(&path).unwrap();
        let conn = Connection::open(&path).unwrap();
        seed_default_calendar(&conn).unwrap();
        conn
    }

    #[test]
    fn set_list_and_clear_day_color() {
        let conn = test_conn();
        assert!(list_day_colors(&conn, "2026-01-01", "2026-12-31", None)
            .unwrap()
            .is_empty());
        set_day_color(&conn, "2026-03-15", Some("green"), None).unwrap();
        let rows = list_day_colors(&conn, "2026-03-01", "2026-03-31", None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].color, "green");
        set_day_color(&conn, "2026-03-15", None, None).unwrap();
        assert!(list_day_colors(&conn, "2026-03-01", "2026-03-31", None)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn set_day_colors_batch() {
        let conn = test_conn();
        set_day_colors(
            &conn,
            &["2026-04-01".into(), "2026-04-02".into()],
            Some("green"),
            None,
        )
        .unwrap();
        assert_eq!(
            list_day_colors(&conn, "2026-04-01", "2026-04-30", None)
                .unwrap()
                .len(),
            2
        );
    }
}
