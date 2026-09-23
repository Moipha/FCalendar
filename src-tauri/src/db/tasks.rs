use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::app_state;
use super::calendars::calendar_has_account;
use super::ics_patch::{task_ics_from_existing, TaskDraft};
use super::queue;
use super::time::now_millis;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRow {
    pub id: String,
    pub calendar_id: String,
    pub summary: String,
    pub description: Option<String>,
    pub is_stamp: bool,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskInput {
    pub calendar_id: Option<String>,
    pub summary: String,
    pub description: Option<String>,
    pub is_stamp: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskInput {
    pub summary: String,
    pub description: Option<String>,
    pub is_stamp: bool,
}

fn map_task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRow> {
    Ok(TaskRow {
        id: row.get(0)?,
        calendar_id: row.get(1)?,
        summary: row.get(2)?,
        description: row.get(3)?,
        is_stamp: row.get::<_, i64>(4)? == 1,
        sort_order: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn next_sort_order(conn: &Connection, calendar_id: &str, is_stamp: bool) -> Result<i64, String> {
    let stamp = i64::from(is_stamp);
    let min: Option<i64> = conn
        .query_row(
            "SELECT MIN(sort_order) FROM tasks WHERE calendar_id = ?1 AND is_stamp = ?2 AND deleted_at IS NULL",
            params![calendar_id, stamp],
            |row| row.get(0),
        )
        .map_err(|e| format!("query min sort_order: {e}"))?;
    Ok(min.map(|v| v - 1).unwrap_or(0))
}

pub fn list_tasks(conn: &Connection, calendar_id: Option<&str>) -> Result<Vec<TaskRow>, String> {
    let cal = match calendar_id {
        Some(id) => id.to_string(),
        None => app_state::current_calendar_id(conn)?,
    };
    let mut stmt = conn
        .prepare(
            "SELECT id, calendar_id, summary, description, is_stamp, sort_order, created_at, updated_at
             FROM tasks
             WHERE calendar_id = ?1 AND deleted_at IS NULL
             ORDER BY is_stamp ASC, sort_order ASC, created_at DESC",
        )
        .map_err(|e| format!("prepare list_tasks: {e}"))?;
    let rows = stmt
        .query_map(params![cal], map_task_row)
        .map_err(|e| format!("query list_tasks: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect list_tasks: {e}"))?;
    Ok(rows)
}

pub fn get_task(conn: &Connection, id: &str) -> Result<TaskRow, String> {
    conn.query_row(
        "SELECT id, calendar_id, summary, description, is_stamp, sort_order, created_at, updated_at
         FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
        map_task_row,
    )
    .map_err(|e| format!("get task: {e}"))
}

pub fn create_task(conn: &Connection, input: CreateTaskInput) -> Result<TaskRow, String> {
    if input.summary.trim().is_empty() {
        return Err("summary is required".into());
    }
    let calendar_id = match input.calendar_id.as_deref() {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => app_state::current_calendar_id(conn)?,
    };
    let id = Uuid::new_v4().to_string();
    let uid = Uuid::new_v4().to_string();
    let now = now_millis();
    let sort_order = next_sort_order(conn, &calendar_id, input.is_stamp)?;
    let dirty = calendar_has_account(conn, &calendar_id)?;
    let ics = task_ics_from_existing(
        None,
        &TaskDraft {
            uid: uid.clone(),
            summary: input.summary.clone(),
            description: input.description.clone(),
            is_stamp: input.is_stamp,
            sort_order,
        },
    )?;
    conn.execute(
        "INSERT INTO tasks (
            id, calendar_id, uid, ics, summary, description, is_stamp, sort_order,
            dirty, created_at, updated_at
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",
        params![
            id,
            calendar_id,
            uid,
            ics,
            input.summary.trim(),
            input.description,
            i64::from(input.is_stamp),
            sort_order,
            i64::from(dirty),
            now,
        ],
    )
    .map_err(|e| format!("insert task: {e}"))?;
    if dirty {
        queue::enqueue(conn, "task", &id, "create")?;
    }
    get_task(conn, &id)
}

pub fn update_task(conn: &Connection, id: &str, input: UpdateTaskInput) -> Result<TaskRow, String> {
    if input.summary.trim().is_empty() {
        return Err("summary is required".into());
    }
    let existing = get_task(conn, id)?;
    let now = now_millis();
    let sort_order = if existing.is_stamp != input.is_stamp {
        next_sort_order(conn, &existing.calendar_id, input.is_stamp)?
    } else {
        existing.sort_order
    };
    let (uid, old_ics): (String, Option<String>) = conn
        .query_row(
            "SELECT uid, ics FROM tasks WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("load task ics: {e}"))?;
    let ics = task_ics_from_existing(
        old_ics.as_deref(),
        &TaskDraft {
            uid,
            summary: input.summary.clone(),
            description: input.description.clone(),
            is_stamp: input.is_stamp,
            sort_order,
        },
    )?;
    let dirty = calendar_has_account(conn, &existing.calendar_id)?;
    conn.execute(
        "UPDATE tasks SET summary=?1, description=?2, is_stamp=?3, sort_order=?4,
            ics=?5, dirty=?6, updated_at=?7
         WHERE id=?8 AND deleted_at IS NULL",
        params![
            input.summary.trim(),
            input.description,
            i64::from(input.is_stamp),
            sort_order,
            ics,
            i64::from(dirty),
            now,
            id,
        ],
    )
    .map_err(|e| format!("update task: {e}"))?;
    if dirty {
        queue::enqueue(conn, "task", id, "update")?;
    }
    get_task(conn, id)
}

pub fn delete_task(conn: &Connection, id: &str) -> Result<(), String> {
    let calendar_id: String = conn
        .query_row(
            "SELECT calendar_id FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| format!("task not found: {e}"))?;
    let remote = calendar_has_account(conn, &calendar_id)?;
    if remote {
        let now = now_millis();
        conn.execute(
            "UPDATE tasks SET deleted_at=?2, dirty=1, updated_at=?2 WHERE id=?1",
            params![id, now],
        )
        .map_err(|e| format!("soft delete task: {e}"))?;
        queue::enqueue(conn, "task", id, "delete")?;
    } else {
        let changed = conn
            .execute("DELETE FROM tasks WHERE id = ?1", params![id])
            .map_err(|e| format!("delete task: {e}"))?;
        if changed == 0 {
            return Err("task not found".into());
        }
    }
    Ok(())
}

pub struct TaskSyncRow {
    pub id: String,
    pub uid: String,
    pub href: Option<String>,
    pub etag: Option<String>,
    pub ics: String,
    pub dirty: bool,
    pub deleted_at: Option<i64>,
    pub summary: String,
    pub description: Option<String>,
    pub is_stamp: bool,
    pub sort_order: i64,
}

pub fn list_sync_tasks(conn: &Connection, calendar_id: &str) -> Result<Vec<TaskSyncRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, uid, href, etag, ics, dirty, deleted_at, summary, description, is_stamp, sort_order
             FROM tasks WHERE calendar_id = ?1",
        )
        .map_err(|e| format!("prepare sync tasks: {e}"))?;
    let rows = stmt
        .query_map(params![calendar_id], |row| {
            Ok(TaskSyncRow {
                id: row.get(0)?,
                uid: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                href: row.get(2)?,
                etag: row.get(3)?,
                ics: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                dirty: row.get::<_, i64>(5)? == 1,
                deleted_at: row.get(6)?,
                summary: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                description: row.get(8)?,
                is_stamp: row.get::<_, i64>(9)? == 1,
                sort_order: row.get(10)?,
            })
        })
        .map_err(|e| format!("query sync tasks: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect sync tasks: {e}"))?;
    Ok(rows)
}

pub fn upsert_remote_task(
    conn: &Connection,
    calendar_id: &str,
    uid: &str,
    href: &str,
    etag: Option<&str>,
    ics: &str,
    summary: &str,
    description: Option<&str>,
    is_stamp: bool,
    sort_order: i64,
) -> Result<(), String> {
    let now = now_millis();
    let existing: Option<String> = match conn.query_row(
        "SELECT id FROM tasks WHERE calendar_id = ?1 AND uid = ?2",
        params![calendar_id, uid],
        |row| row.get(0),
    ) {
        Ok(id) => Some(id),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => return Err(format!("lookup task uid: {e}")),
    };
    if let Some(id) = existing {
        conn.execute(
            "UPDATE tasks SET href=?2, etag=?3, ics=?4, summary=?5, description=?6,
                is_stamp=?7, sort_order=?8, dirty=0, deleted_at=NULL, updated_at=?9
             WHERE id=?1",
            params![
                id,
                href,
                etag,
                ics,
                summary,
                description,
                i64::from(is_stamp),
                sort_order,
                now
            ],
        )
        .map_err(|e| format!("update remote task: {e}"))?;
    } else {
        conn.execute(
            "INSERT INTO tasks (
                id, calendar_id, uid, href, etag, ics, summary, description,
                is_stamp, sort_order, dirty, created_at, updated_at
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,0,?11,?11)",
            params![
                Uuid::new_v4().to_string(),
                calendar_id,
                uid,
                href,
                etag,
                ics,
                summary,
                description,
                i64::from(is_stamp),
                sort_order,
                now
            ],
        )
        .map_err(|e| format!("insert remote task: {e}"))?;
    }
    Ok(())
}

pub fn mark_task_synced(conn: &Connection, id: &str, href: &str, etag: Option<&str>) -> Result<(), String> {
    conn.execute(
        "UPDATE tasks SET href=?2, etag=?3, dirty=0 WHERE id=?1",
        params![id, href, etag],
    )
    .map_err(|e| format!("mark task synced: {e}"))?;
    queue::drop_entity_queue(conn, "task", id)
}

pub fn hard_delete_task(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])
        .map_err(|e| format!("hard delete task: {e}"))?;
    queue::drop_entity_queue(conn, "task", id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::calendars::seed_default_calendar;

    fn mem_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("sql/001_init.sql")).unwrap();
        conn.execute_batch(include_str!("sql/002_tasks.sql")).unwrap();
        conn.execute_batch(include_str!("sql/003_day_colors.sql")).unwrap();
        conn.execute_batch(include_str!("sql/004_sync.sql")).unwrap();
        seed_default_calendar(&conn).unwrap();
        conn
    }

    #[test]
    fn create_and_list_tasks() {
        let conn = mem_conn();
        let task = create_task(
            &conn,
            CreateTaskInput {
                calendar_id: None,
                summary: "Buy milk".into(),
                description: None,
                is_stamp: false,
            },
        )
        .unwrap();
        assert!(!task.is_stamp);
        let stamp = create_task(
            &conn,
            CreateTaskInput {
                calendar_id: None,
                summary: "Go home".into(),
                description: Some("template".into()),
                is_stamp: true,
            },
        )
        .unwrap();
        assert!(stamp.is_stamp);
        assert_eq!(list_tasks(&conn, None).unwrap().len(), 2);
    }

    #[test]
    fn switch_stamp_moves_to_top_sort() {
        let conn = mem_conn();
        let a = create_task(
            &conn,
            CreateTaskInput {
                calendar_id: None,
                summary: "A".into(),
                description: None,
                is_stamp: false,
            },
        )
        .unwrap();
        create_task(
            &conn,
            CreateTaskInput {
                calendar_id: None,
                summary: "B".into(),
                description: None,
                is_stamp: false,
            },
        )
        .unwrap();
        let updated = update_task(
            &conn,
            &a.id,
            UpdateTaskInput {
                summary: "A".into(),
                description: None,
                is_stamp: true,
            },
        )
        .unwrap();
        assert!(updated.is_stamp);
    }
}
