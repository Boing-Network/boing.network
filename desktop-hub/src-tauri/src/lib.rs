#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
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
