mod caldav;
mod credentials;
mod db;
mod sync;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let database = db::Db::open(app).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })?;
            app.manage(database);

            #[cfg(desktop)]
            {
                use tauri::tray::TrayIconBuilder;
                use tauri_plugin_autostart::MacosLauncher;

                app.handle()
                    .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))?;
                app.handle().plugin(tauri_plugin_autostart::init(
                    MacosLauncher::LaunchAgent,
                    None,
                ))?;

                if let Some(icon) = app.default_window_icon() {
                    let _tray = TrayIconBuilder::new().icon(icon.clone()).build(app)?;
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            db::db_health,
            db::list_calendars,
            db::list_events,
            db::get_event,
            db::create_event,
            db::update_event,
            db::delete_event,
            db::list_tasks,
            db::create_task,
            db::update_task,
            db::delete_task,
            db::list_day_colors,
            db::set_day_color,
            db::set_day_colors,
            db::get_account,
            db::connect_account,
            db::disconnect_account_cmd,
            db::rediscover_calendars,
            get_session,
            bootstrap_session,
            sync_current_calendar,
            set_current_calendar,
            update_account_password,
            create_remote_calendar,
            logout_account,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn get_session(db: tauri::State<'_, db::Db>) -> Result<sync::SessionSnapshot, String> {
    db.with_conn(sync::snapshot)
}

#[tauri::command]
async fn bootstrap_session(db: tauri::State<'_, db::Db>) -> Result<sync::SessionSnapshot, String> {
    sync::bootstrap(&db).await
}

#[tauri::command]
async fn sync_current_calendar(db: tauri::State<'_, db::Db>) -> Result<sync::SyncOutcome, String> {
    sync::sync_current(&db).await
}

#[tauri::command]
fn set_current_calendar(
    db: tauri::State<'_, db::Db>,
    calendar_id: String,
) -> Result<sync::SessionSnapshot, String> {
    db.with_conn(|conn| sync::set_current_calendar(conn, &calendar_id))
}

#[tauri::command]
async fn update_account_password(
    db: tauri::State<'_, db::Db>,
    password: String,
) -> Result<sync::SessionSnapshot, String> {
    sync::update_password(&db, &password).await
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCalendarInput {
    display_name: String,
    color: Option<String>,
    migrate_from_local: bool,
    clear_local: bool,
}

#[tauri::command]
async fn create_remote_calendar(
    db: tauri::State<'_, db::Db>,
    input: CreateCalendarInput,
) -> Result<sync::SessionSnapshot, String> {
    sync::create_remote_calendar(
        &db,
        input.display_name,
        input.color,
        input.migrate_from_local,
        input.clear_local,
    )
    .await
}

#[tauri::command]
async fn logout_account(
    db: tauri::State<'_, db::Db>,
    discard: bool,
) -> Result<sync::SessionSnapshot, String> {
    if !discard {
        let pending = db.with_conn(db::queue::has_pending_remote)?;
        if pending {
            return Err("有未同步的改动，请先同步或选择放弃".into());
        }
    }
    db.with_conn(sync::discard_and_logout)
}
