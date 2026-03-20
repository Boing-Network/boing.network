// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    install_panic_hook();
    boing_network_hub::run();
}

/// Write panic and hook errors to a log file so users can diagnose "app won't open".
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        if let Some(log_path) = crash_log_path() {
            let _ = std::fs::create_dir_all(log_path.parent().unwrap_or(std::path::Path::new(".")));
            let msg = format!(
                "{} panic: {}",
                chrono_now(),
                info.to_string()
            );
            let _ = std::fs::write(&log_path, format!("{}\n", msg));
        }
    }));
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{}", t.as_secs())
}

fn crash_log_path() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(|local| {
            std::path::PathBuf::from(local).join("network.boing.hub").join("crash-log.txt")
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}
