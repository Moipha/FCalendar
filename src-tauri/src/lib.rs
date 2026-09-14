#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
