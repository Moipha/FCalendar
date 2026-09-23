use rusqlite::{params, Connection};
use serde::Serialize;
use uuid::Uuid;

use crate::caldav::{DiscoverResult, DiscoveredCalendar};
use crate::credentials::{self, credential_ref_for_account};

use super::app_state;
use super::calendars::CalendarRow;
use super::sqlite_util::map_optional;
use super::time::now_millis;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRow {
    pub id: String,
    pub display_name: String,
    pub server_url: String,
    pub username: String,
    pub principal_url: Option<String>,
    pub calendar_home_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub account: Option<AccountRow>,
    pub remote_calendars: Vec<CalendarRow>,
}

pub fn get_account_status(conn: &Connection) -> Result<AccountStatus, String> {
    let account = get_account(conn)?;
    let remote_calendars = if account.is_some() {
        list_remote_calendars(conn)?
    } else {
        Vec::new()
    };
    Ok(AccountStatus {
        account,
        remote_calendars,
    })
}

pub fn get_account(conn: &Connection) -> Result<Option<AccountRow>, String> {
    let result = conn.query_row(
        "SELECT id, display_name, server_url, username, principal_url, calendar_home_url
         FROM accounts
         LIMIT 1",
        [],
        |row| {
            Ok(AccountRow {
                id: row.get(0)?,
                display_name: row.get(1)?,
                server_url: row.get(2)?,
                username: row.get(3)?,
                principal_url: row.get(4)?,
                calendar_home_url: row.get(5)?,
            })
        },
    );
    map_optional(result).map_err(|e| format!("load account: {e}"))
}

fn list_remote_calendars(conn: &Connection) -> Result<Vec<CalendarRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, account_id, href, display_name, color, visible
             FROM calendars
             WHERE account_id IS NOT NULL
             ORDER BY display_name ASC",
        )
        .map_err(|e| format!("prepare remote calendars: {e}"))?;
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
        .map_err(|e| format!("query remote calendars: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect remote calendars: {e}"))?;
    Ok(rows)
}

pub fn persist_connect(
    conn: &Connection,
    server_url: String,
    username: String,
    password: &str,
    discovery: DiscoverResult,
) -> Result<AccountStatus, String> {
    if get_account(conn)?.is_some() {
        return Err("已有账户，请先断开".into());
    }
    let account_id = Uuid::new_v4().to_string();
    let credential_ref = credential_ref_for_account(&account_id);
    let now = now_millis();
    let display_name = username.clone();

    conn.execute(
        "INSERT INTO accounts (
            id, display_name, server_url, username, credential_ref,
            principal_url, calendar_home_url, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            account_id,
            display_name,
            server_url.trim(),
            username.trim(),
            credential_ref,
            discovery.principal_url,
            discovery.calendar_home_url,
            now,
        ],
    )
    .map_err(|e| format!("insert account: {e}"))?;

    if let Err(e) = credentials::store_password(&credential_ref, password) {
        let _ = conn.execute("DELETE FROM accounts WHERE id = ?1", params![account_id]);
        return Err(e);
    }

    sync_remote_calendars(conn, &account_id, &discovery)?;
    app_state::set_connection_status(conn, "online")?;

    get_account_status(conn)
}

pub fn disconnect_account(conn: &Connection) -> Result<(), String> {
    let Some(account) = get_account(conn)? else {
        return Ok(());
    };
    let credential_ref = credential_ref_for_account(&account.id);
    conn.execute("DELETE FROM accounts WHERE id = ?1", params![account.id])
        .map_err(|e| format!("delete account: {e}"))?;
    let _ = credentials::delete_password(&credential_ref);
    Ok(())
}

pub fn persist_rediscover(
    conn: &Connection,
    discovery: DiscoverResult,
) -> Result<AccountStatus, String> {
    let account = get_account(conn)?.ok_or_else(|| "未登录账户".to_string())?;
    let now = now_millis();
    conn.execute(
        "UPDATE accounts SET principal_url = ?2, calendar_home_url = ?3, updated_at = ?4 WHERE id = ?1",
        params![
            account.id,
            discovery.principal_url,
            discovery.calendar_home_url,
            now,
        ],
    )
    .map_err(|e| format!("update account discovery: {e}"))?;
    sync_remote_calendars(conn, &account.id, &discovery)?;
    get_account_status(conn)
}

fn sync_remote_calendars(
    conn: &Connection,
    account_id: &str,
    discovery: &DiscoverResult,
) -> Result<(), String> {
    let now = now_millis();
    let mut seen_hrefs: Vec<String> = Vec::new();

    for cal in &discovery.calendars {
        seen_hrefs.push(cal.href.clone());
        upsert_remote_calendar(conn, account_id, cal, now)?;
    }

    let mut stmt = conn
        .prepare("SELECT id, href FROM calendars WHERE account_id = ?1")
        .map_err(|e| format!("list account calendars: {e}"))?;
    let rows = stmt
        .query_map(params![account_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("query account calendars: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect account calendars: {e}"))?;
    for (id, href) in rows {
        if !seen_hrefs.iter().any(|h| h == &href) {
            conn.execute("DELETE FROM calendars WHERE id = ?1", params![id])
                .map_err(|e| format!("delete stale calendar: {e}"))?;
        }
    }
    Ok(())
}

fn upsert_remote_calendar(
    conn: &Connection,
    account_id: &str,
    cal: &DiscoveredCalendar,
    now: i64,
) -> Result<(), String> {
    let lookup = conn.query_row(
        "SELECT id FROM calendars WHERE account_id = ?1 AND href = ?2",
        params![account_id, cal.href],
        |row| row.get(0),
    );
    let existing: Option<String> =
        map_optional(lookup).map_err(|e| format!("lookup calendar: {e}"))?;

    let supports_vevent = i64::from(cal.supports_vevent);
    let supports_vtodo = i64::from(cal.supports_vtodo);

    if let Some(id) = existing {
        conn.execute(
            "UPDATE calendars SET
                display_name = ?2, color = ?3, ctag = ?4, sync_token = ?5,
                supports_vevent = ?6, supports_vtodo = ?7, updated_at = ?8
             WHERE id = ?1",
            params![
                id,
                cal.display_name,
                cal.color,
                cal.ctag,
                cal.sync_token,
                supports_vevent,
                supports_vtodo,
                now,
            ],
        )
        .map_err(|e| format!("update remote calendar: {e}"))?;
    } else {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO calendars (
                id, account_id, href, display_name, color, ctag, sync_token,
                supports_vevent, supports_vtodo, visible, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, ?10)",
            params![
                id,
                account_id,
                cal.href,
                cal.display_name,
                cal.color,
                cal.ctag,
                cal.sync_token,
                supports_vevent,
                supports_vtodo,
                now,
            ],
        )
        .map_err(|e| format!("insert remote calendar: {e}"))?;
    }
    Ok(())
}
