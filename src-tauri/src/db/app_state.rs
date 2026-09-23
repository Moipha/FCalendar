use rusqlite::{params, Connection};

use super::calendars::local_calendar_id;
use super::sqlite_util::map_optional;

pub const KEY_CURRENT_CALENDAR: &str = "current_calendar_id";
pub const KEY_CONNECTION: &str = "connection_status";

pub fn get_value(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let result = conn.query_row(
        "SELECT value FROM app_state WHERE key = ?1",
        params![key],
        |row| row.get(0),
    );
    map_optional(result).map_err(|e| format!("app_state get: {e}"))
}

pub fn set_value(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| format!("app_state set: {e}"))?;
    Ok(())
}

pub fn current_calendar_id(conn: &Connection) -> Result<String, String> {
    if let Some(id) = get_value(conn, KEY_CURRENT_CALENDAR)? {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendars WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|e| format!("check calendar: {e}"))?;
        if exists > 0 {
            return Ok(id);
        }
    }
    let local = local_calendar_id(conn)?.ok_or_else(|| "没有本地日历".to_string())?;
    set_value(conn, KEY_CURRENT_CALENDAR, &local)?;
    Ok(local)
}

pub fn set_current_calendar(conn: &Connection, id: &str) -> Result<(), String> {
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM calendars WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| format!("check calendar: {e}"))?;
    if exists == 0 {
        return Err("日历不存在".into());
    }
    set_value(conn, KEY_CURRENT_CALENDAR, id)
}

pub fn connection_status(conn: &Connection) -> Result<String, String> {
    Ok(get_value(conn, KEY_CONNECTION)?.unwrap_or_else(|| "logged_out".into()))
}

pub fn set_connection_status(conn: &Connection, status: &str) -> Result<(), String> {
    set_value(conn, KEY_CONNECTION, status)
}
