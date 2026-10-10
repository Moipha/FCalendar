mod caldav;
mod credentials;
mod db;
mod sync;

use tauri::Manager;

struct FcLog;

impl log::Log for FcLog {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Error
    }
    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            eprintln!("[f-calendar] {}", record.args());
        }
    }
    fn flush(&self) {}
}

static FC_LOG: FcLog = FcLog;

/// 配置窗口创建失败时 Tauri 仍会留下名为 main 的僵尸对象；用 Win32 标题确认真窗口还在。
#[cfg(windows)]
fn fcalendar_hwnd_exists() -> bool {
    type Hwnd = *mut std::ffi::c_void;
    #[link(name = "user32")]
    extern "system" {
        fn FindWindowW(class: *const u16, title: *const u16) -> Hwnd;
    }
    let class: Vec<u16> = "Tauri Window\0".encode_utf16().collect();
    let title: Vec<u16> = "FCalendar\0".encode_utf16().collect();
    unsafe { !FindWindowW(class.as_ptr(), title.as_ptr()).is_null() }
}

/// `%LOCALAPPDATA%` 在本机继承 ACL 只有管理员可写，WebView2 子进程会 `0x8000FFFF`；
/// 进程内 `icacls` 也会失败。优先用可执行文件旁的目录（dev 在 D:`target\debug`，ACL 正常）。
#[cfg(windows)]
fn prepare_webview2_user_data() {
    if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_some() {
        return;
    }

    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("webview2-data"));
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(
            std::path::PathBuf::from(local)
                .join("com.f.cal")
                .join("webview2-data"),
        );
    }

    for dir in candidates {
        if std::fs::create_dir_all(&dir).is_err() {
            continue;
        }
        let probe = dir.join(".fc-write-probe");
        if std::fs::write(&probe, b"ok").is_err() {
            continue;
        }
        let _ = std::fs::remove_file(&probe);
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &dir);
        return;
    }

    eprintln!("[f-calendar] 找不到 WebView2 可写的用户数据目录");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = log::set_logger(&FC_LOG);
    log::set_max_level(log::LevelFilter::Error);
    #[cfg(windows)]
    prepare_webview2_user_data();

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
                    .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }))?;
                app.handle().plugin(tauri_plugin_autostart::init(
                    MacosLauncher::LaunchAgent,
                    None,
                ))?;

                if let Some(icon) = app.default_window_icon() {
                    let _tray = TrayIconBuilder::new().icon(icon.clone()).build(app)?;
                }
            }

            #[cfg(windows)]
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    if !fcalendar_hwnd_exists() {
                        eprintln!(
                            "[f-calendar] 主窗口已消失：WebView2 未能初始化。请查看上方 failed to create webview 的 HRESULT。"
                        );
                        handle.exit(1);
                    }
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) && window.label() == "main" {
                std::process::exit(0);
            }
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
