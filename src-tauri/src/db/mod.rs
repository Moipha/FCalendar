pub(crate) mod accounts;
pub(crate) mod app_state;
pub(crate) mod calendars;
pub(crate) mod day_colors;
pub(crate) mod events;
pub(crate) mod ics;
pub(crate) mod ics_patch;
pub(crate) mod queue;
mod sqlite_util;
pub(crate) mod tasks;
pub(crate) mod time;
pub(crate) use time::now_millis as now_millis_pub;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite_migration::{Migrations, M};
use serde::Serialize;
use std::path::PathBuf;
use tauri::Manager;

pub use accounts::{
    disconnect_account, get_account_status, persist_connect, persist_rediscover, AccountStatus,
};
pub use calendars::CalendarRow;
pub use day_colors::DayColorRow;
pub use events::{EventInstance, EventRow, SaveEventInput};
pub use tasks::{CreateTaskInput, TaskRow, UpdateTaskInput};

pub struct Db {
    pool: Pool<SqliteConnectionManager>,
    path: PathBuf,
}

#[derive(Serialize)]
pub struct DbHealth {
    pub path: String,
    pub ok: bool,
}

impl Db {
    pub fn open(app: &tauri::App) -> Result<Self, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("resolve app data dir: {e}"))?;
        std::fs::create_dir_all(&dir).map_err(|e| format!("create app data dir: {e}"))?;

        let path = dir.join("fcalendar.db");
        migrate(&path)?;

        let manager = SqliteConnectionManager::file(&path).with_init(|conn| {
            conn.execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA busy_timeout = 5000;",
            )
        });

        let pool = Pool::builder()
            .max_size(8)
            .build(manager)
            .map_err(|e| format!("create db pool: {e}"))?;

        let db = Self { pool, path };
        db.with_conn(|conn| {
            calendars::seed_default_calendar(conn)?;
            let _ = app_state::current_calendar_id(conn)?;
            Ok(())
        })?;
        Ok(db)
    }

    pub fn with_conn<T>(&self, f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>) -> Result<T, String> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("get db connection: {e}"))?;
        f(&conn)
    }

    pub fn health(&self) -> Result<DbHealth, String> {
        self.with_conn(|conn| {
            conn.query_row("SELECT 1", [], |_| Ok(()))
                .map_err(|e| format!("ping db: {e}"))?;
            Ok(DbHealth {
                path: self.path.to_string_lossy().into_owned(),
                ok: true,
            })
        })
    }
}

fn migrate(path: &std::path::Path) -> Result<(), String> {
    let mut conn =
        rusqlite::Connection::open(path).map_err(|e| format!("open db for migrate: {e}"))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("enable foreign keys: {e}"))?;

    let migrations = Migrations::new(vec![
        M::up(include_str!("sql/001_init.sql")),
        M::up(include_str!("sql/002_tasks.sql")),
        M::up(include_str!("sql/003_day_colors.sql")),
        M::up(include_str!("sql/004_sync.sql")),
    ]);
    migrations
        .to_latest(&mut conn)
        .map_err(|e| format!("run db migrations: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn db_health(db: tauri::State<Db>) -> Result<DbHealth, String> {
    db.health()
}

#[tauri::command]
pub fn list_calendars(db: tauri::State<Db>) -> Result<Vec<CalendarRow>, String> {
    db.with_conn(calendars::list_calendars)
}

#[tauri::command]
pub fn list_events(
    db: tauri::State<Db>,
    from: String,
    to: String,
    calendar_id: Option<String>,
) -> Result<Vec<EventInstance>, String> {
    db.with_conn(|conn| {
        events::list_event_instances(
            conn,
            &from,
            &to,
            calendar_id.as_deref(),
        )
    })
}

#[tauri::command]
pub fn get_event(db: tauri::State<Db>, id: String) -> Result<EventRow, String> {
    db.with_conn(|conn| events::get_event(conn, &id))
}

#[tauri::command]
pub fn create_event(db: tauri::State<Db>, input: SaveEventInput) -> Result<EventRow, String> {
    db.with_conn(|conn| events::create_event(conn, input))
}

#[tauri::command]
pub fn update_event(
    db: tauri::State<Db>,
    id: String,
    input: SaveEventInput,
) -> Result<EventRow, String> {
    db.with_conn(|conn| events::update_event(conn, &id, input))
}

#[tauri::command]
pub fn delete_event(db: tauri::State<Db>, id: String) -> Result<(), String> {
    db.with_conn(|conn| events::delete_event(conn, &id))
}

#[tauri::command]
pub fn list_tasks(
    db: tauri::State<Db>,
    calendar_id: Option<String>,
) -> Result<Vec<TaskRow>, String> {
    db.with_conn(|conn| tasks::list_tasks(conn, calendar_id.as_deref()))
}

#[tauri::command]
pub fn create_task(db: tauri::State<Db>, input: CreateTaskInput) -> Result<TaskRow, String> {
    db.with_conn(|conn| tasks::create_task(conn, input))
}

#[tauri::command]
pub fn update_task(
    db: tauri::State<Db>,
    id: String,
    input: UpdateTaskInput,
) -> Result<TaskRow, String> {
    db.with_conn(|conn| tasks::update_task(conn, &id, input))
}

#[tauri::command]
pub fn delete_task(db: tauri::State<Db>, id: String) -> Result<(), String> {
    db.with_conn(|conn| tasks::delete_task(conn, &id))
}

#[tauri::command]
pub fn list_day_colors(
    db: tauri::State<Db>,
    from: String,
    to: String,
    calendar_id: Option<String>,
) -> Result<Vec<DayColorRow>, String> {
    db.with_conn(|conn| day_colors::list_day_colors(conn, &from, &to, calendar_id.as_deref()))
}

#[tauri::command]
pub fn set_day_color(
    db: tauri::State<Db>,
    date: String,
    color: Option<String>,
    calendar_id: Option<String>,
) -> Result<(), String> {
    db.with_conn(|conn| {
        day_colors::set_day_color(conn, &date, color.as_deref(), calendar_id.as_deref())
    })
}

#[tauri::command]
pub fn set_day_colors(
    db: tauri::State<Db>,
    dates: Vec<String>,
    color: Option<String>,
    calendar_id: Option<String>,
) -> Result<(), String> {
    db.with_conn(|conn| {
        day_colors::set_day_colors(conn, &dates, color.as_deref(), calendar_id.as_deref())
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectAccountInput {
    pub server_url: String,
    pub username: String,
    pub password: String,
}

#[tauri::command]
pub fn get_account(db: tauri::State<'_, Db>) -> Result<AccountStatus, String> {
    db.with_conn(get_account_status)
}

#[tauri::command]
pub async fn connect_account(
    db: tauri::State<'_, Db>,
    input: ConnectAccountInput,
) -> Result<AccountStatus, String> {
    let discovery = crate::caldav::discover(
        &input.server_url,
        &input.username,
        &input.password,
    )
    .await?;
    let status = db.with_conn(|conn| {
        persist_connect(
            conn,
            input.server_url,
            input.username,
            &input.password,
            discovery,
        )
    })?;
    if status.account.is_some() {
        let _ = crate::sync::sync_with_password(&db, &input.password).await;
    }
    Ok(status)
}

#[tauri::command]
pub fn disconnect_account_cmd(db: tauri::State<'_, Db>) -> Result<(), String> {
    db.with_conn(disconnect_account)
}

#[tauri::command]
pub async fn rediscover_calendars(db: tauri::State<'_, Db>) -> Result<AccountStatus, String> {
    let (server_url, username, password) = db.with_conn(|conn| {
        let account = get_account_status(conn)?;
        let account = account
            .account
            .ok_or_else(|| "未登录账户".to_string())?;
        let credential_ref = crate::credentials::credential_ref_for_account(&account.id);
        let password = crate::credentials::load_password(&credential_ref)?;
        Ok((account.server_url, account.username, password))
    })?;
    let discovery = match crate::caldav::discover(&server_url, &username, &password).await {
        Ok(discovery) => discovery,
        Err(e) => {
            db.with_conn(|conn| crate::sync::apply_error_status(conn, &e, true))?;
            return Err(e);
        }
    };
    db.with_conn(|conn| {
        persist_rediscover(conn, discovery)?;
        crate::db::app_state::set_connection_status(conn, "online")?;
        get_account_status(conn)
    })
}

#[cfg(test)]
mod tests {
    use super::migrate;

    #[test]
    fn init_migration_creates_core_tables() {
        let dir = std::env::temp_dir().join(format!("fcalendar-db-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.db");
        let _ = std::fs::remove_file(&path);

        migrate(&path).unwrap();

        let conn = rusqlite::Connection::open(&path).unwrap();
        for table in [
            "accounts",
            "calendars",
            "events",
            "todos",
            "tasks",
            "day_colors",
            "alarms",
            "memos",
            "change_queue",
            "conflicts",
            "app_state",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing table {table}");
        }

        let _ = std::fs::remove_dir_all(dir);
    }
}
