use rusqlite::Connection;
use url::Url;

use crate::caldav::{
    absolute_url, collection_url, list_collection_objects, normalize_server_url, CaldavClient,
    CollectionObject,
};
use crate::credentials;
use crate::db::accounts::{get_account, persist_rediscover, AccountRow};
use crate::db::app_state;
use crate::db::calendars::{self, CalendarRow};
use crate::db::day_colors;
use crate::db::events;
use crate::db::ics_patch::{classify_ics, RemoteObject};
use crate::db::queue;
use crate::db::tasks;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub current_calendar_id: String,
    pub connection_status: String,
    pub pending_remote_changes: bool,
    pub account: Option<AccountRow>,
    pub calendars: Vec<CalendarRow>,
}

pub fn snapshot(conn: &Connection) -> Result<SessionSnapshot, String> {
    let account = get_account(conn)?;
    if account.is_none() {
        app_state::set_connection_status(conn, "logged_out")?;
    }
    Ok(SessionSnapshot {
        current_calendar_id: app_state::current_calendar_id(conn)?,
        connection_status: app_state::connection_status(conn)?,
        pending_remote_changes: queue::has_pending_remote(conn)?,
        account,
        calendars: calendars::list_calendars(conn)?,
    })
}

pub async fn bootstrap(db: &crate::db::Db) -> Result<SessionSnapshot, String> {
    let account = db.with_conn(get_account)?;
    let Some(account) = account else {
        return db.with_conn(|conn| {
            app_state::set_connection_status(conn, "logged_out")?;
            snapshot(conn)
        });
    };
    match load_password_and_sync(db, &account, None).await {
        Ok(()) => {}
        Err(kind) => {
            db.with_conn(|conn| {
                app_state::set_connection_status(conn, kind)?;
                Ok(())
            })?;
        }
    }
    db.with_conn(snapshot)
}

async fn load_password_and_sync(
    db: &crate::db::Db,
    account: &AccountRow,
    current_override: Option<String>,
) -> Result<(), &'static str> {
    let password = credentials::load_password(&credentials::credential_ref_for_account(&account.id))
        .map_err(|_| "offline")?;
    try_connect_and_sync(db, account, &password, current_override).await
}

async fn try_connect_and_sync(
    db: &crate::db::Db,
    account: &AccountRow,
    password: &str,
    current_override: Option<String>,
) -> Result<(), &'static str> {
    let client = CaldavClient::new(&account.username, password).map_err(|_| "offline")?;
    let base = normalize_server_url(&account.server_url, &account.username).map_err(|_| "offline")?;
    let probe_url = match &account.calendar_home_url {
        Some(home) => absolute_url(&base, home).map_err(|_| "offline")?,
        None => base.to_string(),
    };
    match client
        .propfind(
            &probe_url,
            0,
            r#"<?xml version="1.0"?><D:propfind xmlns:D="DAV:"><D:prop><D:displayname/></D:prop></D:propfind>"#,
        )
        .await
    {
        Ok(_) => {}
        Err(e) => return Err(classify_net(&e)),
    }
    db.with_conn(|conn| app_state::set_connection_status(conn, "online"))
        .map_err(|_| "offline")?;
    let calendar_id = db
        .with_conn(|conn| {
            Ok(current_override.unwrap_or(app_state::current_calendar_id(conn)?))
        })
        .map_err(|_| "offline")?;
    if let Err(e) = sync_calendar(db, &client, &base, &calendar_id).await {
        return Err(classify_net(&e));
    }
    Ok(())
}

fn classify_net(e: &str) -> &'static str {
    // 只认我们自己的 HTTP 状态文案，避免 URL / 响应体里的数字误判。
    if e.contains("返回 401")
        || e.contains("返回 403")
        || e.contains("失败 401")
        || e.contains("失败 403")
    {
        "auth_error"
    } else {
        "offline"
    }
}

pub async fn sync_current(db: &crate::db::Db) -> Result<SessionSnapshot, String> {
    let account = db.with_conn(get_account)?.ok_or_else(|| "未登录".to_string())?;
    match load_password_and_sync(db, &account, None).await {
        Ok(()) => db.with_conn(snapshot),
        Err(kind) => {
            db.with_conn(|conn| {
                app_state::set_connection_status(conn, kind)?;
                snapshot(conn)
            })?;
            if kind == "auth_error" {
                Err("密码无效，请到设置重新输入".into())
            } else {
                db.with_conn(snapshot)
            }
        }
    }
}

async fn sync_calendar(
    db: &crate::db::Db,
    client: &CaldavClient,
    base: &Url,
    calendar_id: &str,
) -> Result<(), String> {
    let cal = db.with_conn(|conn| calendars::get_calendar(conn, calendar_id))?;
    if cal.account_id.is_none() {
        return Ok(());
    }
    let cal_url = collection_url(base, &cal.href)?;
    let remote = list_collection_objects(client, &cal_url).await?;
    pull_remote(db, calendar_id, &remote)?;
    push_local(db, client, &cal_url, calendar_id).await?;
    Ok(())
}

fn pull_remote(
    db: &crate::db::Db,
    calendar_id: &str,
    remote: &[CollectionObject],
) -> Result<(), String> {
    db.with_conn(|conn| {
        let local_events = events::list_sync_events(conn, calendar_id)?;
        let local_tasks = tasks::list_sync_tasks(conn, calendar_id)?;
        let local_colors = day_colors::list_sync_colors(conn, calendar_id)?;

        let mut remote_event_uids = Vec::new();
        let mut remote_task_uids = Vec::new();
        let mut remote_color_dates = Vec::new();

        for obj in remote {
            let Some(parsed) = classify_ics(&obj.ics) else {
                continue;
            };
            match parsed {
                RemoteObject::Event {
                    uid,
                    summary,
                    description,
                    all_day,
                    dtstart,
                    dtend,
                    rrule,
                    ics,
                } => {
                    remote_event_uids.push(uid.clone());
                    let local = local_events.iter().find(|e| e.uid == uid);
                    if local.map(|e| e.dirty).unwrap_or(false) {
                        continue;
                    }
                    events::upsert_remote_event(
                        conn,
                        calendar_id,
                        &uid,
                        &obj.href,
                        obj.etag.as_deref(),
                        &ics,
                        &summary,
                        description.as_deref(),
                        all_day,
                        &dtstart,
                        dtend.as_deref(),
                        rrule.as_deref(),
                    )?;
                }
                RemoteObject::Task {
                    uid,
                    summary,
                    description,
                    is_stamp,
                    sort_order,
                    ics,
                } => {
                    remote_task_uids.push(uid.clone());
                    let local = local_tasks.iter().find(|t| t.uid == uid);
                    if local.map(|t| t.dirty).unwrap_or(false) {
                        continue;
                    }
                    tasks::upsert_remote_task(
                        conn,
                        calendar_id,
                        &uid,
                        &obj.href,
                        obj.etag.as_deref(),
                        &ics,
                        &summary,
                        description.as_deref(),
                        is_stamp,
                        sort_order,
                    )?;
                }
                RemoteObject::DayColor { date, color, ics } => {
                    remote_color_dates.push(date.clone());
                    let local = local_colors.iter().find(|c| c.date == date);
                    if local.map(|c| c.dirty).unwrap_or(false) {
                        continue;
                    }
                    day_colors::upsert_remote_color(
                        conn,
                        calendar_id,
                        &date,
                        &obj.href,
                        obj.etag.as_deref(),
                        &ics,
                        &color,
                    )?;
                }
            }
        }

        for ev in local_events {
            if ev.dirty || ev.deleted_at.is_some() {
                continue;
            }
            if !remote_event_uids.iter().any(|u| u == &ev.uid) {
                events::hard_delete_event(conn, &ev.id)?;
            }
        }
        for t in local_tasks {
            if t.dirty || t.deleted_at.is_some() {
                continue;
            }
            if !remote_task_uids.iter().any(|u| u == &t.uid) {
                tasks::hard_delete_task(conn, &t.id)?;
            }
        }
        for c in local_colors {
            if c.dirty || c.deleted_at.is_some() {
                continue;
            }
            if !remote_color_dates.iter().any(|d| d == &c.date) {
                day_colors::hard_delete_color(conn, calendar_id, &c.date)?;
            }
        }
        Ok(())
    })
}

async fn push_local(
    db: &crate::db::Db,
    client: &CaldavClient,
    cal_url: &str,
    calendar_id: &str,
) -> Result<(), String> {
    let events = db.with_conn(|conn| events::list_sync_events(conn, calendar_id))?;
    for ev in events {
        if ev.deleted_at.is_some() {
            push_delete(client, cal_url, ev.href.as_deref(), ev.etag.as_deref(), &ev.uid).await?;
            db.with_conn(|conn| events::hard_delete_event(conn, &ev.id))?;
            continue;
        }
        if !ev.dirty {
            continue;
        }
        let (href, etag) = push_put(
            client,
            cal_url,
            ev.href.as_deref(),
            ev.etag.as_deref(),
            &ev.uid,
            &ev.ics,
        )
        .await?;
        db.with_conn(|conn| events::mark_event_synced(conn, &ev.id, &href, etag.as_deref()))?;
    }

    let task_rows = db.with_conn(|conn| tasks::list_sync_tasks(conn, calendar_id))?;
    for t in task_rows {
        if t.deleted_at.is_some() {
            push_delete(client, cal_url, t.href.as_deref(), t.etag.as_deref(), &t.uid).await?;
            db.with_conn(|conn| tasks::hard_delete_task(conn, &t.id))?;
            continue;
        }
        if !t.dirty {
            continue;
        }
        let (href, etag) =
            push_put(client, cal_url, t.href.as_deref(), t.etag.as_deref(), &t.uid, &t.ics).await?;
        db.with_conn(|conn| tasks::mark_task_synced(conn, &t.id, &href, etag.as_deref()))?;
    }

    let colors = db.with_conn(|conn| day_colors::list_sync_colors(conn, calendar_id))?;
    for c in colors {
        if c.deleted_at.is_some() {
            push_delete(client, cal_url, c.href.as_deref(), c.etag.as_deref(), &c.uid).await?;
            db.with_conn(|conn| day_colors::hard_delete_color(conn, calendar_id, &c.date))?;
            continue;
        }
        if !c.dirty {
            continue;
        }
        let (href, etag) =
            push_put(client, cal_url, c.href.as_deref(), c.etag.as_deref(), &c.uid, &c.ics).await?;
        db.with_conn(|conn| {
            day_colors::mark_color_synced(conn, calendar_id, &c.date, &href, etag.as_deref())
        })?;
    }
    Ok(())
}

fn object_url(cal_url: &str, href: Option<&str>, uid: &str) -> String {
    let dir = if cal_url.ends_with('/') {
        cal_url.to_string()
    } else {
        format!("{cal_url}/")
    };
    // 只取文件名，避免旧的错误 href（写在账户根下）再次 join 出集合外。
    let file = href
        .and_then(|h| {
            h.rsplit('/')
                .find(|s| !s.is_empty())
                .filter(|s| s.ends_with(".ics"))
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| format!("{uid}.ics"));
    format!("{dir}{file}")
}

async fn push_put(
    client: &CaldavClient,
    cal_url: &str,
    href: Option<&str>,
    etag: Option<&str>,
    uid: &str,
    ics: &str,
) -> Result<(String, Option<String>), String> {
    let url = object_url(cal_url, href, uid);
    let (status, new_etag) = client.put(&url, ics, etag).await?;
    if status == 412 {
        let (status2, etag2) = client.put(&url, ics, None).await?;
        if status2 >= 400 {
            return Err(format!("PUT 后写失败 {status2}"));
        }
        return Ok((url, etag2));
    }
    if status >= 400 {
        return Err(format!("PUT 失败 {status}"));
    }
    Ok((url, new_etag.or_else(|| etag.map(|s| s.to_string()))))
}

async fn push_delete(
    client: &CaldavClient,
    cal_url: &str,
    href: Option<&str>,
    etag: Option<&str>,
    uid: &str,
) -> Result<(), String> {
    let url = object_url(cal_url, href, uid);
    let status = client.delete(&url, etag).await?;
    if status == 404 || status == 412 {
        let status2 = client.delete(&url, None).await?;
        if status2 >= 400 && status2 != 404 {
            return Err(format!("DELETE 失败 {status2}"));
        }
        return Ok(());
    }
    if status >= 400 {
        return Err(format!("DELETE 失败 {status}"));
    }
    Ok(())
}

pub async fn update_password(db: &crate::db::Db, password: &str) -> Result<SessionSnapshot, String> {
    let account = db.with_conn(get_account)?.ok_or_else(|| "未登录".to_string())?;
    credentials::store_password(
        &credentials::credential_ref_for_account(&account.id),
        password,
    )?;
    match try_connect_and_sync(db, &account, password, None).await {
        Ok(()) => db.with_conn(snapshot),
        Err("auth_error") => {
            db.with_conn(|conn| app_state::set_connection_status(conn, "auth_error"))?;
            Err("密码无效，请到设置重新输入".into())
        }
        Err(kind) => {
            db.with_conn(|conn| {
                app_state::set_connection_status(conn, kind)?;
                snapshot(conn)
            })
        }
    }
}

pub async fn create_remote_calendar(
    db: &crate::db::Db,
    display_name: String,
    color: Option<String>,
    migrate_from_local: bool,
    clear_local: bool,
) -> Result<SessionSnapshot, String> {
    let status = db.with_conn(app_state::connection_status)?;
    if status != "online" {
        return Err("离线时不能新建日历本".into());
    }
    let account = db.with_conn(get_account)?.ok_or_else(|| "未登录".to_string())?;
    let password = credentials::load_password(&credentials::credential_ref_for_account(&account.id))?;
    let client = CaldavClient::new(&account.username, &password)?;
    let base = normalize_server_url(&account.server_url, &account.username)?;
    let home = match &account.calendar_home_url {
        Some(h) => collection_url(&base, h)?,
        None => return Err("缺少 calendar-home".into()),
    };
    let slug = slugify(&display_name);
    let url = format!("{}/{slug}/", home.trim_end_matches('/'));
    client
        .mkcalendar(&url, &display_name, color.as_deref())
        .await?;
    let discovery = crate::caldav::discover(&account.server_url, &account.username, &password).await?;
    db.with_conn(|conn| persist_rediscover(conn, discovery))?;
    let new_id = db.with_conn(|conn| {
        let cals = calendars::list_calendars(conn)?;
        cals.into_iter()
            .filter(|c| c.account_id.is_some())
            .find(|c| c.href.contains(&slug) || c.display_name == display_name)
            .map(|c| c.id)
            .ok_or_else(|| "新建后未找到日历".to_string())
    })?;
    if migrate_from_local {
        db.with_conn(|conn| migrate_local_into(conn, &new_id, clear_local))?;
    }
    db.with_conn(|conn| app_state::set_current_calendar(conn, &new_id))?;
    let _ = load_password_and_sync(db, &account, Some(new_id)).await;
    db.with_conn(snapshot)
}

fn slugify(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        format!("cal-{}", crate::db::now_millis_pub())
    } else {
        s
    }
}

fn migrate_local_into(conn: &Connection, dest_id: &str, clear_local: bool) -> Result<(), String> {
    let local_id = calendars::local_calendar_id(conn)?.ok_or_else(|| "没有本地日历".to_string())?;
    copy_events(conn, &local_id, dest_id)?;
    copy_tasks(conn, &local_id, dest_id)?;
    copy_colors(conn, &local_id, dest_id)?;
    if clear_local {
        conn.execute("DELETE FROM events WHERE calendar_id = ?1", rusqlite::params![local_id])
            .map_err(|e| format!("清空本地事件: {e}"))?;
        conn.execute("DELETE FROM tasks WHERE calendar_id = ?1", rusqlite::params![local_id])
            .map_err(|e| format!("清空本地任务: {e}"))?;
        conn.execute("DELETE FROM day_colors WHERE calendar_id = ?1", rusqlite::params![local_id])
            .map_err(|e| format!("清空本地颜色: {e}"))?;
    }
    Ok(())
}

fn copy_events(conn: &Connection, from: &str, to: &str) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT uid, ics, summary, description, dtstart, dtend, all_day, rrule FROM events
             WHERE calendar_id = ?1 AND deleted_at IS NULL",
        )
        .map_err(|e| format!("copy events: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![from], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })
        .map_err(|e| format!("query copy events: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect copy events: {e}"))?;
    let now = crate::db::now_millis_pub();
    for (uid, ics, summary, description, dtstart, dtend, all_day, rrule) in rows {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO events (
                id, calendar_id, uid, ics, summary, description, dtstart, dtend, all_day, rrule,
                dirty, created_at, updated_at
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,1,?11,?11)",
            rusqlite::params![
                id, to, uid, ics, summary, description, dtstart, dtend, all_day, rrule, now
            ],
        )
        .map_err(|e| format!("insert copied event: {e}"))?;
        queue::enqueue(conn, "event", &id, "create")?;
    }
    Ok(())
}

fn copy_tasks(conn: &Connection, from: &str, to: &str) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT uid, ics, summary, description, is_stamp, sort_order FROM tasks
             WHERE calendar_id = ?1 AND deleted_at IS NULL",
        )
        .map_err(|e| format!("copy tasks: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![from], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|e| format!("query copy tasks: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect copy tasks: {e}"))?;
    let now = crate::db::now_millis_pub();
    for (uid, ics, summary, description, is_stamp, sort_order) in rows {
        let id = uuid::Uuid::new_v4().to_string();
        let uid = uid.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        conn.execute(
            "INSERT INTO tasks (
                id, calendar_id, uid, ics, summary, description, is_stamp, sort_order,
                dirty, created_at, updated_at
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1,?9,?9)",
            rusqlite::params![id, to, uid, ics, summary, description, is_stamp, sort_order, now],
        )
        .map_err(|e| format!("insert copied task: {e}"))?;
        queue::enqueue(conn, "task", &id, "create")?;
    }
    Ok(())
}

fn copy_colors(conn: &Connection, from: &str, to: &str) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT date, color, uid, ics FROM day_colors WHERE calendar_id = ?1 AND deleted_at IS NULL",
        )
        .map_err(|e| format!("copy colors: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![from], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| format!("query copy colors: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect copy colors: {e}"))?;
    for (date, color, uid, ics) in rows {
        conn.execute(
            "INSERT INTO day_colors (calendar_id, date, color, uid, ics, dirty, deleted_at)
             VALUES (?1,?2,?3,?4,?5,1,NULL)",
            rusqlite::params![to, date, color, uid, ics],
        )
        .map_err(|e| format!("insert copied color: {e}"))?;
        queue::enqueue(conn, "day_color", &format!("{to}:{date}"), "create")?;
    }
    Ok(())
}

pub fn set_current_calendar(conn: &Connection, id: &str) -> Result<SessionSnapshot, String> {
    app_state::set_current_calendar(conn, id)?;
    snapshot(conn)
}

/// 登录刚写入钥匙串后，用用户刚输入的密码探测/同步，避免立刻再读钥匙串失败被当成密码错。
pub async fn sync_with_password(db: &crate::db::Db, password: &str) -> Result<(), &'static str> {
    let account = db
        .with_conn(get_account)
        .map_err(|_| "offline")?
        .ok_or("offline")?;
    try_connect_and_sync(db, &account, password, None).await
}

#[cfg(test)]
mod tests {
    use super::object_url;

    #[test]
    fn object_url_joins_relative_href() {
        let url = object_url(
            "http://192.168.1.8:5232/alice/work/",
            Some("/alice/work/evt.ics"),
            "evt",
        );
        assert_eq!(url, "http://192.168.1.8:5232/alice/work/evt.ics");
    }

    #[test]
    fn object_url_builds_from_uid() {
        let url = object_url("http://192.168.1.8:5232/alice/work/", None, "abc");
        assert_eq!(url, "http://192.168.1.8:5232/alice/work/abc.ics");
    }

    #[test]
    fn object_url_keeps_collection_when_base_has_no_slash() {
        let url = object_url("http://192.168.1.8:5232/moipha/work", None, "abc");
        assert_eq!(url, "http://192.168.1.8:5232/moipha/work/abc.ics");
    }

    #[test]
    fn object_url_relocates_href_that_landed_in_home() {
        let url = object_url(
            "http://192.168.1.8:5232/moipha/work/",
            Some("/moipha/abc.ics"),
            "abc",
        );
        assert_eq!(url, "http://192.168.1.8:5232/moipha/work/abc.ics");
    }
}

pub fn discard_and_logout(conn: &Connection) -> Result<SessionSnapshot, String> {
    crate::db::accounts::disconnect_account(conn)?;
    queue::clear_all_queue(conn)?;
    let local = calendars::local_calendar_id(conn)?.ok_or_else(|| "没有本地日历".to_string())?;
    app_state::set_current_calendar(conn, &local)?;
    app_state::set_connection_status(conn, "logged_out")?;
    snapshot(conn)
}
