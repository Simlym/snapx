use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};

mod capture;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(commands::PinStore(Mutex::new(HashMap::new())))
        .manage(commands::CurrentShortcut(Mutex::new(None)))
        .setup(|app| {
            // ── System tray menu ──────────────────────────────────────
            let capture_item     = MenuItem::with_id(app, "capture",          "📸 区域截图 (Ctrl+Shift+X)", true, None::<&str>)?;
            let window_item      = MenuItem::with_id(app, "capture-window",   "🪟 窗口截图",               true, None::<&str>)?;
            let fullscreen_item  = MenuItem::with_id(app, "capture-fullscreen","🖥️ 全屏截图",              true, None::<&str>)?;
            let delay2_item      = MenuItem::with_id(app, "capture-delay-2",  "⏱ 延迟 2 秒截图",          true, None::<&str>)?;
            let delay5_item      = MenuItem::with_id(app, "capture-delay-5",  "⏱ 延迟 5 秒截图",          true, None::<&str>)?;
            let paste_pin_item   = MenuItem::with_id(app, "paste-pin",        "📋 粘贴为贴图",             true, None::<&str>)?;
            let close_pins_item  = MenuItem::with_id(app, "close-all-pins",   "❌ 关闭全部贴图",           true, None::<&str>)?;
            let settings_item    = MenuItem::with_id(app, "settings",         "⚙  设置",                  true, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let sep3 = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[
                &capture_item, &window_item, &fullscreen_item,
                &sep1,
                &delay2_item, &delay5_item,
                &sep2,
                &paste_pin_item, &close_pins_item,
                &sep3,
                &settings_item, &quit_item,
            ])?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("SnapX - Screenshot Tool")
                .menu(&menu)
                .on_menu_event(move |app_handle, event| {
                    let ah = app_handle.clone();
                    match event.id().as_ref() {
                        "capture" => {
                            tauri::async_runtime::spawn(async move {
                                let _ = ah.emit("trigger-capture", "region");
                            });
                        }
                        "capture-window" => {
                            tauri::async_runtime::spawn(async move {
                                let _ = ah.emit("trigger-capture", "window");
                            });
                        }
                        "capture-fullscreen" => {
                            tauri::async_runtime::spawn(async move {
                                let _ = ah.emit("trigger-capture", "fullscreen");
                            });
                        }
                        "capture-delay-2" => {
                            tauri::async_runtime::spawn(async move {
                                let _ = ah.emit("trigger-capture", "delayed-2");
                            });
                        }
                        "capture-delay-5" => {
                            tauri::async_runtime::spawn(async move {
                                let _ = ah.emit("trigger-capture", "delayed-5");
                            });
                        }
                        "paste-pin" => {
                            tauri::async_runtime::spawn(async move {
                                let _ = ah.emit("paste-pin", ());
                            });
                        }
                        "close-all-pins" => {
                            tauri::async_runtime::spawn(async move {
                                let _ = ah.emit("close-all-pins", ());
                            });
                        }
                        "settings" => {
                            tauri::async_runtime::spawn(async move {
                                let _ = ah.emit("show-settings", ());
                            });
                        }
                        "quit" => app_handle.exit(0),
                        _ => {}
                    }
                })
                .build(app)?;

            // ── Global shortcut ───────────────────────────────────────
            use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

            let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyX);
            let shortcut_for_state = shortcut.clone();
            let _ = app.global_shortcut().unregister(shortcut.clone());
            if let Err(e) = app.global_shortcut().on_shortcut(shortcut, |app_handle, _event, _shortcut| {
                let ah = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = ah.emit("trigger-capture", "region");
                });
            }) {
                eprintln!("Warning: failed to register global shortcut: {e}");
            }

            *app.state::<commands::CurrentShortcut>()
                .0.lock().unwrap() = Some(shortcut_for_state);

            // ── Hide windows at startup ───────────────────────────────
            if let Some(w) = app.get_webview_window("main")    { w.hide()?; }
            if let Some(w) = app.get_webview_window("overlay") { w.hide()?; }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::capture_screens,
            commands::capture_region,
            commands::capture_window,
            commands::save_to_clipboard,
            commands::save_to_file,
            commands::save_to_file_dialog,
            commands::save_to_quicksave,
            commands::read_clipboard_image,
            commands::list_monitors,
            commands::store_pin_data,
            commands::get_pin_data,
            commands::remove_pin_data,
            commands::update_global_shortcut,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SnapX");
}
