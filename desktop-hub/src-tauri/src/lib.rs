use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

#[tauri::command]
async fn close_splash_and_show_main(app: tauri::AppHandle) -> Result<(), String> {
    let splash = app
        .get_webview_window("splashscreen")
        .ok_or("splash window not found")?;
    let main_win = app
        .get_webview_window("main")
        .ok_or("main window not found")?;
    splash.close().map_err(|e| e.to_string())?;
    main_win.show().map_err(|e| e.to_string())?;
    main_win.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// Second instance was launched; surface the existing Hub (splash or main, e.g. from tray).
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn focus_running_hub_instance(app: &tauri::AppHandle) {
    if let Some(splash) = app.get_webview_window("splashscreen") {
        let _ = splash.show();
        let _ = splash.set_focus();
        return;
    }
    show_main_window(app);
}

const TRAY_MENU_SHOW: &str = "tray_show";
const TRAY_MENU_QUIT: &str = "tray_quit";

fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(
        app,
        TRAY_MENU_SHOW,
        "Show Boing Network Hub",
        true,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, TRAY_MENU_QUIT, "Quit", true, None::<&str>)?;
    let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let mut tray = TrayIconBuilder::with_id("hub_tray")
        .menu(&tray_menu)
        // Left-click restores the window; use the menu (e.g. right-click on Windows) for Quit.
        .show_menu_on_left_click(false)
        .tooltip(
            "Boing Network Hub — closing the window keeps the hub in the tray. Click to reopen, or use Quit to exit.",
        );

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    let tray_click_app = app.clone();

    tray.on_menu_event(move |app, event| {
        if event.id() == TRAY_MENU_SHOW {
            show_main_window(app);
        } else if event.id() == TRAY_MENU_QUIT {
            app.exit(0);
        }
    })
    .on_tray_icon_event(move |_tray, event| match event {
        TrayIconEvent::Click {
            button,
            button_state,
            ..
        } if button == MouseButton::Left && button_state == MouseButtonState::Up => {
            show_main_window(&tray_click_app);
        }
        TrayIconEvent::DoubleClick { .. } => {
            show_main_window(&tray_click_app);
        }
        _ => {}
    })
    .build(app)?;

    Ok(())
}

/// Hide the main window to the system tray instead of exiting when the user closes it.
/// Registered on [`tauri::Builder::on_window_event`] so it always runs with the window pipeline
/// (attaching only in `.setup()` on the webview can miss the hook on some platforms).
fn main_window_close_to_tray(window: &tauri::Window, event: &WindowEvent) {
    if window.label() != "main" {
        return;
    }
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    api.prevent_close();
    let _ = window.hide();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_running_hub_instance(app);
        }));
    }

    builder
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .on_window_event(main_window_close_to_tray)
        .invoke_handler(tauri::generate_handler![close_splash_and_show_main])
        .setup(|app| {
            // On Windows, ensure app data dir exists so WebView2 can create EBWebView (see tauri-apps/tauri#12787)
            #[cfg(target_os = "windows")]
            if let Ok(data_dir) = app.path().app_data_dir() {
                let _ = std::fs::create_dir_all(&data_dir);
            }

            let handle = app.handle().clone();
            setup_tray(&handle)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Boing Network Hub");
}
