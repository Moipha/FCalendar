use rusqlite::{params, Connection};
use uuid::Uuid;

use super::time::now_millis;

pub fn enqueue(conn: &Connection, entity_type: &str, entity_id: &str, op: &str) -> Result<(), String> {
    let now = now_millis();
    conn.execute(
        "DELETE FROM change_queue WHERE entity_type = ?1 AND entity_id = ?2 AND status = 'pending'",
        params![entity_type, entity_id],
    )
    .map_err(|e| format!("clear pending queue: {e}"))?;
    conn.execute(
        "INSERT INTO change_queue (
            id, entity_type, entity_id, op, status, attempts, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, 'pending', 0, ?5, ?5)",
        params![Uuid::new_v4().to_string(), entity_type, entity_id, op, now],
    )
    .map_err(|e| format!("enqueue: {e}"))?;
    Ok(())
}

pub fn has_pending_remote(conn: &Connection) -> Result<bool, String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM change_queue WHERE status IN ('pending', 'in_flight', 'failed')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("count queue: {e}"))?;
    if n > 0 {
        return Ok(true);
    }
    let dirty_events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM events e
             JOIN calendars c ON c.id = e.calendar_id
             WHERE c.account_id IS NOT NULL AND (e.dirty = 1 OR e.deleted_at IS NOT NULL)",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("count dirty events: {e}"))?;
    let dirty_tasks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks t
             JOIN calendars c ON c.id = t.calendar_id
             WHERE c.account_id IS NOT NULL AND (t.dirty = 1 OR t.deleted_at IS NOT NULL)",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("count dirty tasks: {e}"))?;
    let dirty_colors: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM day_colors d
             JOIN calendars c ON c.id = d.calendar_id
             WHERE c.account_id IS NOT NULL AND (d.dirty = 1 OR d.deleted_at IS NOT NULL)",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("count dirty colors: {e}"))?;
    Ok(dirty_events + dirty_tasks + dirty_colors > 0)
}

pub fn list_pending(
    conn: &Connection,
    calendar_id: &str,
) -> Result<Vec<(String, String, String)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT q.entity_type, q.entity_id, q.op
             FROM change_queue q
             WHERE q.status IN ('pending', 'failed')
             ORDER BY q.created_at ASC",
        )
        .map_err(|e| format!("prepare queue: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("query queue: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect queue: {e}"))?;

    let mut out = Vec::new();
    for (ty, id, op) in rows {
        let belongs = match ty.as_str() {
            "event" => row_calendar(conn, "events", &id)?,
            "task" => row_calendar(conn, "tasks", &id)?,
            "day_color" => Some(id.split_once(':').map(|(c, _)| c.to_string()).unwrap_or(id.clone())),
            _ => None,
        };
        if belongs.as_deref() == Some(calendar_id) {
            out.push((ty, id, op));
        }
    }
    Ok(out)
}

fn row_calendar(conn: &Connection, table: &str, id: &str) -> Result<Option<String>, String> {
    let sql = format!("SELECT calendar_id FROM {table} WHERE id = ?1");
    match conn.query_row(&sql, params![id], |row| row.get::<_, String>(0)) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("lookup {table}: {e}")),
    }
}

pub fn drop_entity_queue(conn: &Connection, entity_type: &str, entity_id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM change_queue WHERE entity_type = ?1 AND entity_id = ?2",
        params![entity_type, entity_id],
    )
    .map_err(|e| format!("drop queue: {e}"))?;
    Ok(())
}

pub fn clear_all_queue(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM change_queue", [])
        .map_err(|e| format!("clear queue: {e}"))?;
    Ok(())
}
