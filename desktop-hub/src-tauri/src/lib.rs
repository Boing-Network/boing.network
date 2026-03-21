use tauri::Manager;

#[tauri::command]
fn show_main_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![show_main_window])
        .setup(|app| {
            // On Windows, ensure app data dir exists so WebView2 can create EBWebView (see tauri-apps/tauri#12787)
            #[cfg(target_os = "windows")]
            if let Ok(data_dir) = app.path().app_data_dir() {
                let _ = std::fs::create_dir_all(&data_dir);
            }
            // Show main window on next tick (window may not exist yet in setup)
            let app_handle = app.handle().clone();
            let _ = app_handle.clone().run_on_main_thread(move || {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Boing Network Hub");
}
