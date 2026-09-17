use rusqlite::{params, Connection};
use serde::Serialize;
use uuid::Uuid;

use super::time::now_millis;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarRow {
    pub id: String,
    pub account_id: Option<String>,
    pub href: String,
    pub display_name: String,
    pub color: Option<String>,
    pub visible: bool,
}

pub fn seed_default_calendar(conn: &Connection) -> Result<(), String> {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM calendars", [], |row| row.get(0))
        .map_err(|e| format!("count calendars: {e}"))?;
    if count > 0 {
        return Ok(());
    }

    let id = Uuid::new_v4().to_string();
    let now = now_millis();
    conn.execute(
        "INSERT INTO calendars (
            id, account_id, href, display_name, color, supports_vevent, supports_vtodo,
            visible, created_at, updated_at
        ) VALUES (?1, NULL, ?2, '本地', NULL, 1, 0, 1, ?3, ?3)",
        params![id, format!("local:{id}"), now],
    )
    .map_err(|e| format!("seed local calendar: {e}"))?;
    Ok(())
}

pub fn list_calendars(conn: &Connection) -> Result<Vec<CalendarRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, account_id, href, display_name, color, visible
             FROM calendars
             ORDER BY created_at ASC",
        )
        .map_err(|e| format!("prepare list calendars: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(CalendarRow {
                id: row.get(0)?,
                account_id: row.get(1)?,
                href: row.get(2)?,
                display_name: row.get(3)?,
                color: row.get(4)?,
                visible: row.get::<_, i64>(5)? == 1,
            })
        })
        .map_err(|e| format!("query calendars: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect calendars: {e}"))?;
    Ok(rows)
}

pub fn calendar_has_account(conn: &Connection, calendar_id: &str) -> Result<bool, String> {
    let account_id: Option<String> = conn
        .query_row(
            "SELECT account_id FROM calendars WHERE id = ?1",
            params![calendar_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("load calendar account: {e}"))?;
    Ok(account_id.is_some())
}
