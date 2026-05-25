use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager,
};

mod capture;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // ── System tray menu ──────────────────────────────────────
            // Build the tray menu programmatically (Tauri v2 style).
            // This is more portable than the JSON config approach.
            let capture_item = MenuItem::with_id(app, "capture", "📸 区域截图 (Ctrl+Shift+A)", true, None::<&str>)?;
            let window_item = MenuItem::with_id(app, "capture-window", "🪟 窗口截图", true, None::<&str>)?;
            let fullscreen_item = MenuItem::with_id(app, "capture-fullscreen", "🖥️ 全屏截图", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&capture_item, &window_item, &fullscreen_item, &separator, &quit_item])?;

            // Build tray icon — the icon file must exist in the bundle
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("SnapX - Screenshot Tool")
                .menu(&menu)
                .on_menu_event(move |app_handle, event| {
                    match event.id().as_ref() {
                        "capture" => {
                            let ah = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = ah.emit("trigger-capture", "region");
                            });
                        }
                        "capture-window" => {
                            let ah = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = ah.emit("trigger-capture", "window");
                            });
                        }
                        "capture-fullscreen" => {
                            let ah = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = ah.emit("trigger-capture", "fullscreen");
                            });
                        }
                        "quit" => {
                            app_handle.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // ── Global shortcut ───────────────────────────────────────
            // Register Ctrl+Shift+A as the screenshot hotkey.
            use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

            let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyA);
            app.global_shortcut().on_shortcut(shortcut, |app_handle, _event, _shortcut| {
                let ah = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = ah.emit("trigger-capture", "region");
                });
            })?;

            // ── Hide all windows at startup ───────────────────────────
            // The main window and overlay are both hidden; they show only when capturing.
            if let Some(main_win) = app.get_webview_window("main") {
                main_win.hide()?;
            }
            if let Some(overlay_win) = app.get_webview_window("overlay") {
                overlay_win.hide()?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::capture_screens,
            commands::capture_region,
            commands::capture_window,
            commands::save_to_clipboard,
            commands::save_to_file,
            commands::list_monitors,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SnapX");
}
