use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite_migration::{Migrations, M};
use serde::Serialize;
use std::path::PathBuf;
use tauri::Manager;

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

        Ok(Self { pool, path })
    }

    pub fn health(&self) -> Result<DbHealth, String> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("get db connection: {e}"))?;
        conn.query_row("SELECT 1", [], |_| Ok(()))
            .map_err(|e| format!("ping db: {e}"))?;
        Ok(DbHealth {
            path: self.path.to_string_lossy().into_owned(),
            ok: true,
        })
    }
}

fn migrate(path: &std::path::Path) -> Result<(), String> {
    let mut conn =
        rusqlite::Connection::open(path).map_err(|e| format!("open db for migrate: {e}"))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("enable foreign keys: {e}"))?;

    let migrations = Migrations::new(vec![M::up(include_str!("sql/001_init.sql"))]);
    migrations
        .to_latest(&mut conn)
        .map_err(|e| format!("run db migrations: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn db_health(db: tauri::State<Db>) -> Result<DbHealth, String> {
    db.health()
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
            "alarms",
            "memos",
            "change_queue",
            "conflicts",
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
