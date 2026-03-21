use tauri::Manager;

#[tauri::command]
async fn close_splash_and_show_main(app: tauri::AppHandle) -> Result<(), String> {
    let splash = app
        .get_webview_window("splashscreen")
        .ok_or("splash window not found")?;
    let main_win = app.get_webview_window("main").ok_or("main window not found")?;
    splash.close().map_err(|e| e.to_string())?;
    main_win.show().map_err(|e| e.to_string())?;
    main_win.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![close_splash_and_show_main])
        .setup(|app| {
            // On Windows, ensure app data dir exists so WebView2 can create EBWebView (see tauri-apps/tauri#12787)
            #[cfg(target_os = "windows")]
            if let Ok(data_dir) = app.path().app_data_dir() {
                let _ = std::fs::create_dir_all(&data_dir);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Boing Network Hub");
}
