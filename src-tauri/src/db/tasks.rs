use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::time::now_millis;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRow {
    pub id: String,
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
        summary: row.get(1)?,
        description: row.get(2)?,
        is_stamp: row.get::<_, i64>(3)? == 1,
        sort_order: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn next_sort_order(conn: &Connection, is_stamp: bool) -> Result<i64, String> {
    let stamp = i64::from(is_stamp);
    let min: Option<i64> = conn
        .query_row(
            "SELECT MIN(sort_order) FROM tasks WHERE is_stamp = ?1",
            params![stamp],
            |row| row.get(0),
        )
        .map_err(|e| format!("query min sort_order: {e}"))?;

    Ok(min.map(|v| v - 1).unwrap_or(0))
}

pub fn list_tasks(conn: &Connection) -> Result<Vec<TaskRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, summary, description, is_stamp, sort_order, created_at, updated_at
             FROM tasks
             ORDER BY is_stamp ASC, sort_order ASC, created_at DESC",
        )
        .map_err(|e| format!("prepare list_tasks: {e}"))?;

    let rows = stmt
        .query_map([], map_task_row)
        .map_err(|e| format!("query list_tasks: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect list_tasks: {e}"))?;
    Ok(rows)
}

pub fn get_task(conn: &Connection, id: &str) -> Result<TaskRow, String> {
    conn.query_row(
        "SELECT id, summary, description, is_stamp, sort_order, created_at, updated_at
         FROM tasks WHERE id = ?1",
        params![id],
        map_task_row,
    )
    .map_err(|e| format!("get task: {e}"))
}

pub fn create_task(conn: &Connection, input: CreateTaskInput) -> Result<TaskRow, String> {
    if input.summary.trim().is_empty() {
        return Err("summary is required".into());
    }

    let id = Uuid::new_v4().to_string();
    let now = now_millis();
    let sort_order = next_sort_order(conn, input.is_stamp)?;

    conn.execute(
        "INSERT INTO tasks (id, summary, description, is_stamp, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![
            id,
            input.summary.trim(),
            input.description,
            i64::from(input.is_stamp),
            sort_order,
            now,
        ],
    )
    .map_err(|e| format!("insert task: {e}"))?;

    get_task(conn, &id)
}

pub fn update_task(conn: &Connection, id: &str, input: UpdateTaskInput) -> Result<TaskRow, String> {
    if input.summary.trim().is_empty() {
        return Err("summary is required".into());
    }

    let existing = get_task(conn, id)?;
    let now = now_millis();

    let sort_order = if existing.is_stamp != input.is_stamp {
        next_sort_order(conn, input.is_stamp)?
    } else {
        existing.sort_order
    };

    conn.execute(
        "UPDATE tasks
         SET summary = ?1, description = ?2, is_stamp = ?3, sort_order = ?4, updated_at = ?5
         WHERE id = ?6",
        params![
            input.summary.trim(),
            input.description,
            i64::from(input.is_stamp),
            sort_order,
            now,
            id,
        ],
    )
    .map_err(|e| format!("update task: {e}"))?;

    get_task(conn, id)
}

pub fn delete_task(conn: &Connection, id: &str) -> Result<(), String> {
    let changed = conn
        .execute("DELETE FROM tasks WHERE id = ?1", params![id])
        .map_err(|e| format!("delete task: {e}"))?;
    if changed == 0 {
        return Err("task not found".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn mem_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("sql/002_tasks.sql")).unwrap();
        conn
    }

    #[test]
    fn create_and_list_tasks() {
        let conn = mem_conn();
        let task = create_task(
            &conn,
            CreateTaskInput {
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
                summary: "Go home".into(),
                description: Some("template".into()),
                is_stamp: true,
            },
        )
        .unwrap();
        assert!(stamp.is_stamp);

        let all = list_tasks(&conn).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn switch_stamp_moves_to_top_sort() {
        let conn = mem_conn();
        let a = create_task(
            &conn,
            CreateTaskInput {
                summary: "A".into(),
                description: None,
                is_stamp: false,
            },
        )
        .unwrap();
        let b = create_task(
            &conn,
            CreateTaskInput {
                summary: "B".into(),
                description: None,
                is_stamp: false,
            },
        )
        .unwrap();
        assert!(a.sort_order > b.sort_order);

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
        assert_eq!(updated.sort_order, 0);
    }
}
