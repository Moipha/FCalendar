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

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOutcome {
    pub snapshot: SessionSnapshot,
    pub pushed: u32,
    pub pulled: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct SyncStats {
    pushed: u32,
    pulled: u32,
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
    let _ = sync_all_remote_calendars(db, &account).await;
    db.with_conn(snapshot)
}

fn is_auth_error(e: &str) -> bool {
    // 只认我们自己的 HTTP 状态文案，避免 URL / 响应体里的数字误判。
    e.contains("返回 401") || e.contains("返回 403") || e.contains("失败 401") || e.contains("失败 403")
}

fn is_unreachable(e: &str) -> bool {
    e.contains("请求失败")
        || e.contains("创建 HTTP")
        || e.contains("error sending request")
        || e.contains("connection refused")
        || e.contains("timed out")
        || e.contains("os error")
}

/// 探测失败：鉴权红 / 不可达灰。探测已成功后的内容同步失败不改在线。
pub fn apply_error_status(conn: &Connection, error: &str, probe_failed: bool) -> Result<(), String> {
    if is_auth_error(error) {
        return app_state::set_connection_status(conn, "auth_error");
    }
    if probe_failed || is_unreachable(error) {
        return app_state::set_connection_status(conn, "offline");
    }
    Ok(())
}

async fn load_password_and_sync(
    db: &crate::db::Db,
    account: &AccountRow,
    current_override: Option<String>,
) -> Result<SyncStats, String> {
    let password = match credentials::load_password(&credentials::credential_ref_for_account(
        &account.id,
    )) {
        Ok(password) => password,
        Err(e) => {
            db.with_conn(|conn| app_state::set_connection_status(conn, "auth_error"))?;
            return Err(format!("无法读取已存密码：{e}"));
        }
    };
    try_connect_and_sync(db, account, &password, current_override).await
}

async fn try_connect_and_sync(
    db: &crate::db::Db,
    account: &AccountRow,
    password: &str,
    current_override: Option<String>,
) -> Result<SyncStats, String> {
    let client = CaldavClient::new(&account.username, password)?;
    let base = normalize_server_url(&account.server_url, &account.username)?;
    let probe_url = match &account.calendar_home_url {
        Some(home) => absolute_url(&base, home)?,
        None => base.to_string(),
    };
    if let Err(e) = client
        .propfind(
            &probe_url,
            0,
            r#"<?xml version="1.0"?><D:propfind xmlns:D="DAV:"><D:prop><D:displayname/></D:prop></D:propfind>"#,
        )
        .await
    {
        db.with_conn(|conn| apply_error_status(conn, &e, true))?;
        return Err(e);
    }
    db.with_conn(|conn| app_state::set_connection_status(conn, "online"))?;
    let calendar_id = db.with_conn(|conn| {
        Ok(current_override.unwrap_or(app_state::current_calendar_id(conn)?))
    })?;
    match sync_calendar(db, &client, &base, &calendar_id).await {
        Ok(stats) => Ok(stats),
        Err(e) => {
            db.with_conn(|conn| apply_error_status(conn, &e, false))?;
            Err(e)
        }
    }
}

async fn sync_all_remote_calendars(
    db: &crate::db::Db,
    account: &AccountRow,
) -> Result<SyncStats, String> {
    let password = match credentials::load_password(&credentials::credential_ref_for_account(
        &account.id,
    )) {
        Ok(password) => password,
        Err(e) => {
            db.with_conn(|conn| app_state::set_connection_status(conn, "auth_error"))?;
            return Err(format!("无法读取已存密码：{e}"));
        }
    };
    let client = CaldavClient::new(&account.username, &password)?;
    let base = normalize_server_url(&account.server_url, &account.username)?;
    let probe_url = match &account.calendar_home_url {
        Some(home) => absolute_url(&base, home)?,
        None => base.to_string(),
    };
    if let Err(e) = client
        .propfind(
            &probe_url,
            0,
            r#"<?xml version="1.0"?><D:propfind xmlns:D="DAV:"><D:prop><D:displayname/></D:prop></D:propfind>"#,
        )
        .await
    {
        db.with_conn(|conn| apply_error_status(conn, &e, true))?;
        return Err(e);
    }
    db.with_conn(|conn| app_state::set_connection_status(conn, "online"))?;
    let ids = db.with_conn(|conn| {
        Ok(calendars::list_calendars(conn)?
            .into_iter()
            .filter(|c| c.account_id.is_some())
            .map(|c| c.id)
            .collect::<Vec<_>>())
    })?;
    let mut stats = SyncStats::default();
    let mut last_err = None;
    for id in ids {
        match sync_calendar(db, &client, &base, &id).await {
            Ok(s) => {
                stats.pushed += s.pushed;
                stats.pulled += s.pulled;
            }
            Err(e) => last_err = Some(e),
        }
    }
    if let Some(e) = last_err {
        db.with_conn(|conn| apply_error_status(conn, &e, false))?;
        return Err(e);
    }
    Ok(stats)
}

pub async fn sync_current(db: &crate::db::Db) -> Result<SyncOutcome, String> {
    let account = db.with_conn(get_account)?.ok_or_else(|| "未登录".to_string())?;
    match load_password_and_sync(db, &account, None).await {
        Ok(stats) => Ok(SyncOutcome {
            snapshot: db.with_conn(snapshot)?,
            pushed: stats.pushed,
            pulled: stats.pulled,
        }),
        Err(e) => {
            let status = db.with_conn(app_state::connection_status)?;
            if status == "auth_error" {
                Err("密码无效，请到设置重新输入".into())
            } else {
                Err(e)
            }
        }
    }
}

async fn sync_calendar(
    db: &crate::db::Db,
    client: &CaldavClient,
    base: &Url,
    calendar_id: &str,
) -> Result<SyncStats, String> {
    let cal = db.with_conn(|conn| calendars::get_calendar(conn, calendar_id))?;
    if cal.account_id.is_none() {
        return Ok(SyncStats::default());
    }
    let cal_url = collection_url(base, &cal.href)?;
    let remote = list_collection_objects(client, &cal_url).await?;
    let pulled = pull_remote(db, calendar_id, &remote)?;
    let pushed = push_local(db, client, &cal_url, calendar_id).await?;
    Ok(SyncStats { pushed, pulled })
}

fn pull_remote(
    db: &crate::db::Db,
    calendar_id: &str,
    remote: &[CollectionObject],
) -> Result<u32, String> {
    db.with_conn(|conn| {
        let local_events = events::list_sync_events(conn, calendar_id)?;
        let local_tasks = tasks::list_sync_tasks(conn, calendar_id)?;
        let local_colors = day_colors::list_sync_colors(conn, calendar_id)?;

        let mut pulled = 0u32;
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
                    pulled += 1;
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
                    pulled += 1;
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
                    pulled += 1;
                }
            }
        }

        for ev in local_events {
            if ev.dirty || ev.deleted_at.is_some() || ev.href.is_none() {
                continue;
            }
            if !remote_event_uids.iter().any(|u| u == &ev.uid) {
                events::hard_delete_event(conn, &ev.id)?;
                pulled += 1;
            }
        }
        for t in local_tasks {
            if t.dirty || t.deleted_at.is_some() || t.href.is_none() {
                continue;
            }
            if !remote_task_uids.iter().any(|u| u == &t.uid) {
                tasks::hard_delete_task(conn, &t.id)?;
                pulled += 1;
            }
        }
        for c in local_colors {
            if c.dirty || c.deleted_at.is_some() || c.href.is_none() {
                continue;
            }
            if !remote_color_dates.iter().any(|d| d == &c.date) {
                day_colors::hard_delete_color(conn, calendar_id, &c.date)?;
                pulled += 1;
            }
        }
        Ok(pulled)
    })
}

fn needs_push(dirty: bool, href: Option<&str>, deleted_at: Option<i64>) -> bool {
    deleted_at.is_some() || dirty || href.is_none()
}

async fn push_local(
    db: &crate::db::Db,
    client: &CaldavClient,
    cal_url: &str,
    calendar_id: &str,
) -> Result<u32, String> {
    let mut pushed = 0u32;
    let events = db.with_conn(|conn| events::list_sync_events(conn, calendar_id))?;
    for ev in events {
        if ev.deleted_at.is_some() {
            push_delete(client, cal_url, ev.href.as_deref(), ev.etag.as_deref(), &ev.uid).await?;
            db.with_conn(|conn| events::hard_delete_event(conn, &ev.id))?;
            pushed += 1;
            continue;
        }
        if !needs_push(ev.dirty, ev.href.as_deref(), ev.deleted_at) {
            continue;
        }
        let ics = ensure_event_ics(&ev)?;
        let (href, etag) = push_put(
            client,
            cal_url,
            ev.href.as_deref(),
            ev.etag.as_deref(),
            &ev.uid,
            &ics,
        )
        .await?;
        db.with_conn(|conn| events::mark_event_synced(conn, &ev.id, &href, etag.as_deref()))?;
        pushed += 1;
    }

    let task_rows = db.with_conn(|conn| tasks::list_sync_tasks(conn, calendar_id))?;
    for t in task_rows {
        if t.deleted_at.is_some() {
            push_delete(client, cal_url, t.href.as_deref(), t.etag.as_deref(), &t.uid).await?;
            db.with_conn(|conn| tasks::hard_delete_task(conn, &t.id))?;
            pushed += 1;
            continue;
        }
        if !needs_push(t.dirty, t.href.as_deref(), t.deleted_at) {
            continue;
        }
        let ics = ensure_task_ics(&t)?;
        let (href, etag) =
            push_put(client, cal_url, t.href.as_deref(), t.etag.as_deref(), &t.uid, &ics).await?;
        db.with_conn(|conn| tasks::mark_task_synced(conn, &t.id, &href, etag.as_deref()))?;
        pushed += 1;
    }

    let colors = db.with_conn(|conn| day_colors::list_sync_colors(conn, calendar_id))?;
    for c in colors {
        if c.deleted_at.is_some() {
            push_delete(client, cal_url, c.href.as_deref(), c.etag.as_deref(), &c.uid).await?;
            db.with_conn(|conn| day_colors::hard_delete_color(conn, calendar_id, &c.date))?;
            pushed += 1;
            continue;
        }
        if !needs_push(c.dirty, c.href.as_deref(), c.deleted_at) {
            continue;
        }
        let ics = ensure_color_ics(&c)?;
        let (href, etag) =
            push_put(client, cal_url, c.href.as_deref(), c.etag.as_deref(), &c.uid, &ics).await?;
        db.with_conn(|conn| {
            day_colors::mark_color_synced(conn, calendar_id, &c.date, &href, etag.as_deref())
        })?;
        pushed += 1;
    }
    Ok(pushed)
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

fn ensure_event_ics(ev: &events::EventSyncRow) -> Result<String, String> {
    let summary = if ev.summary.trim().is_empty() {
        "（无标题）"
    } else {
        ev.summary.as_str()
    };
    let existing = ev.ics.trim();
    crate::db::ics_patch::event_ics_from_existing(
        (!existing.is_empty()).then_some(existing),
        &crate::db::ics::draft_from_row(
            &ev.uid,
            summary,
            ev.description.as_deref(),
            ev.all_day,
            &ev.dtstart,
            ev.dtend.as_deref(),
            ev.rrule.as_deref(),
        ),
    )
}

fn ensure_task_ics(task: &tasks::TaskSyncRow) -> Result<String, String> {
    if !task.ics.trim().is_empty() {
        return Ok(task.ics.clone());
    }
    crate::db::ics_patch::task_ics_from_existing(
        None,
        &crate::db::ics_patch::TaskDraft {
            uid: task.uid.clone(),
            summary: if task.summary.trim().is_empty() {
                "（无标题）".into()
            } else {
                task.summary.clone()
            },
            description: task.description.clone(),
            is_stamp: task.is_stamp,
            sort_order: task.sort_order,
        },
    )
}

fn ensure_color_ics(color: &day_colors::ColorSyncRow) -> Result<String, String> {
    if !color.ics.trim().is_empty() {
        return Ok(color.ics.clone());
    }
    crate::db::ics_patch::day_color_ics_from_existing(
        None,
        &crate::db::ics_patch::DayColorDraft {
            date: color.date.clone(),
            color: color.color.clone(),
        },
    )
}

fn put_fail_message(url: &str, status: u16, body: &str, ics: &str) -> String {
    let detail = if body.trim().is_empty() {
        "(无响应体)".to_string()
    } else {
        body.chars().take(400).collect()
    };
    let preview = if ics.trim().is_empty() {
        "(空)".to_string()
    } else {
        ics.chars().take(240).collect()
    };
    format!("PUT {url} 失败 {status}: {detail} | ICS: {preview}")
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
    if ics.trim().is_empty() {
        return Err(format!("PUT {url} 失败: 对象 {uid} 的 ICS 为空"));
    }
    let (status, new_etag, body) = client.put(&url, ics, etag).await?;
    if status == 412 {
        let (status2, etag2, body2) = client.put(&url, ics, None).await?;
        if status2 >= 400 {
            return Err(put_fail_message(&url, status2, &body2, ics));
        }
        if let Err(e) = client.get(&url).await {
            return Err(format!(
                "PUT {url} 返回 {status2}，但随后 GET 校验失败: {e}"
            ));
        }
        return Ok((url, etag2));
    }
    if status >= 400 {
        return Err(put_fail_message(&url, status, &body, ics));
    }
    if let Err(e) = client.get(&url).await {
        return Err(format!(
            "PUT {url} 返回 {status}，但随后 GET 校验失败: {e}"
        ));
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
    let (status, body) = client.delete(&url, etag).await?;
    if status == 404 || status == 412 {
        let (status2, body2) = client.delete(&url, None).await?;
        if status2 >= 400 && status2 != 404 {
            return Err(format!(
                "DELETE {url} 失败 {status2}: {}",
                if body2.trim().is_empty() {
                    "(无响应体)".to_string()
                } else {
                    body2.chars().take(400).collect()
                }
            ));
        }
        return Ok(());
    }
    if status >= 400 {
        return Err(format!(
            "DELETE {url} 失败 {status}: {}",
            if body.trim().is_empty() {
                "(无响应体)".to_string()
            } else {
                body.chars().take(400).collect()
            }
        ));
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
        Ok(_) => db.with_conn(snapshot),
        Err(e) => {
            if is_auth_error(&e) {
                Err("密码无效，请到设置重新输入".into())
            } else {
                Err(e)
            }
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
    if let Err(e) = try_connect_and_sync(db, &account, &password, Some(new_id)).await {
        return Err(format!("日历已创建，但同步到服务器失败：{e}"));
    }
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
        let ics = if ics.trim().is_empty() {
            let title = summary
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("（无标题）");
            crate::db::ics_patch::event_ics_from_existing(
                None,
                &crate::db::ics::draft_from_row(
                    &uid,
                    title,
                    description.as_deref(),
                    all_day != 0,
                    &dtstart,
                    dtend.as_deref(),
                    rrule.as_deref(),
                ),
            )?
        } else {
            ics
        };
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
        let ics = match ics.filter(|s| !s.trim().is_empty()) {
            Some(value) => value,
            None => crate::db::ics_patch::task_ics_from_existing(
                None,
                &crate::db::ics_patch::TaskDraft {
                    uid: uid.clone(),
                    summary: summary.clone(),
                    description: description.clone(),
                    is_stamp: is_stamp != 0,
                    sort_order,
                },
            )?,
        };
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
        let ics = match ics.filter(|s| !s.trim().is_empty()) {
            Some(value) => value,
            None => crate::db::ics_patch::day_color_ics_from_existing(
                None,
                &crate::db::ics_patch::DayColorDraft {
                    date: date.clone(),
                    color: color.clone(),
                },
            )?,
        };
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
pub async fn sync_with_password(db: &crate::db::Db, _password: &str) -> Result<(), String> {
    let account = db
        .with_conn(get_account)
        .map_err(|_| "offline")?
        .ok_or("offline")?;
    sync_all_remote_calendars(db, &account).await?;
    Ok(())
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

    #[test]
    fn classifies_auth_and_unreachable() {
        assert!(super::is_auth_error("PROPFIND http://x 返回 401: no"));
        assert!(super::is_unreachable("PROPFIND 请求失败: error sending request"));
        assert!(!super::is_unreachable("PUT 失败 415"));
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
