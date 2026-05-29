use crate::capture;
use base64::Engine;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
use image::GenericImageView;

// ── App-managed state ─────────────────────────────────────────────────────────

pub struct PinStore(pub Mutex<HashMap<String, String>>);

/// Tracks the currently registered global shortcut so we can swap it at runtime.
pub struct CurrentShortcut(pub Mutex<Option<Shortcut>>);

// ── Capture commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn capture_screens(monitor_index: Option<usize>) -> Result<capture::CaptureResult, String> {
    let idx = monitor_index.unwrap_or(0);
    capture::capture_full_monitor(idx)
}

#[tauri::command]
pub async fn capture_region(
    monitor_index: usize,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<capture::CaptureResult, String> {
    capture::capture_monitor_region(monitor_index, x, y, width, height)
}

#[tauri::command]
pub async fn capture_window(monitor_index: Option<usize>) -> Result<capture::CaptureResult, String> {
    let idx = monitor_index.unwrap_or(0);
    capture::capture_full_monitor(idx)
}

#[tauri::command]
pub async fn list_monitors() -> Result<Vec<capture::MonitorInfo>, String> {
    Ok(capture::get_monitor_list())
}

/// List visible windows on a monitor with monitor-local physical-pixel bounds,
/// used by the overlay for hover edge-detection and snap-to-window selection.
#[tauri::command]
pub async fn list_windows(monitor_index: Option<usize>) -> Result<Vec<capture::WindowInfo>, String> {
    capture::list_windows_for_monitor(monitor_index.unwrap_or(0))
}

// ── Clipboard / file commands ─────────────────────────────────────────────────

#[tauri::command]
pub async fn save_to_clipboard(
    app: AppHandle,
    image_data: String,
) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&image_data)
        .map_err(|e| e.to_string())?;

    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let (width, height) = img.dimensions();
    let pixels = img.to_rgba8().into_raw();
    let tauri_image = tauri::image::Image::new_owned(pixels, width, height);

    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_image(&tauri_image).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_to_file(
    image_data: String,
    path: String,
) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&image_data)
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_to_file_dialog(app: AppHandle, image_data: String) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&image_data)
        .map_err(|e| e.to_string())?;

    let maybe_path = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("PNG 图片", &["png"])
            .add_filter("JPEG 图片", &["jpg", "jpeg"])
            .set_file_name("screenshot.png")
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    match maybe_path {
        Some(tauri_plugin_dialog::FilePath::Path(path_buf)) => {
            let is_jpeg = path_buf.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
                .unwrap_or(false);

            let write_bytes = if is_jpeg {
                let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
                let mut buf = std::io::Cursor::new(Vec::new());
                img.write_to(&mut buf, image::ImageFormat::Jpeg).map_err(|e| e.to_string())?;
                buf.into_inner()
            } else {
                bytes
            };
            std::fs::write(&path_buf, write_bytes).map_err(|e| e.to_string())?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

// ── Quick save (auto-named, no dialog) ───────────────────────────────

fn quicksave_dir() -> Result<std::path::PathBuf, String> {
    let base = if cfg!(windows) {
        std::env::var("USERPROFILE").ok()
    } else {
        std::env::var("HOME").ok()
    }
    .map(|h| std::path::PathBuf::from(h).join("Pictures").join("SnapX"))
    .unwrap_or_else(|| std::path::PathBuf::from("."));
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    Ok(base)
}

#[tauri::command]
pub async fn save_to_quicksave(image_data: String) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&image_data)
        .map_err(|e| e.to_string())?;
    let dir = quicksave_dir()?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = dir.join(format!("SnapX_{ts}.png"));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

// ── Read clipboard image ──────────────────────────────────────────────

#[tauri::command]
pub async fn read_clipboard_image(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    match app.clipboard().read_image() {
        Ok(img) => {
            let pixels = img.rgba().to_vec();
            let width  = img.width();
            let height = img.height();
            let rgba = image::RgbaImage::from_raw(width, height, pixels)
                .ok_or("Invalid clipboard image data")?;
            let dynamic = image::DynamicImage::ImageRgba8(rgba);
            let mut buf = std::io::Cursor::new(Vec::new());
            dynamic.write_to(&mut buf, image::ImageFormat::Png).map_err(|e| e.to_string())?;
            Ok(Some(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                buf.into_inner(),
            )))
        }
        Err(_) => Ok(None),
    }
}

// ── Pin window data store ─────────────────────────────────────────────────────

#[tauri::command]
pub fn store_pin_data(
    state: tauri::State<PinStore>,
    id: String,
    data: String,
) -> Result<(), String> {
    state.0.lock().map_err(|e| e.to_string())?.insert(id, data);
    Ok(())
}

#[tauri::command]
pub fn get_pin_data(
    state: tauri::State<PinStore>,
    id: String,
) -> Result<Option<String>, String> {
    Ok(state.0.lock().map_err(|e| e.to_string())?.get(&id).cloned())
}

#[tauri::command]
pub fn remove_pin_data(
    state: tauri::State<PinStore>,
    id: String,
) -> Result<(), String> {
    state.0.lock().map_err(|e| e.to_string())?.remove(&id);
    Ok(())
}

// ── Global shortcut management ────────────────────────────────────────────────

/// Replace the active global shortcut at runtime.
/// `mods` is an array of modifier names: "Ctrl", "Shift", "Alt", "Super".
/// `key` is a single uppercase letter (A-Z), digit (0-9), or Fn key (F1-F12).
#[tauri::command]
pub fn update_global_shortcut(
    app: AppHandle,
    state: tauri::State<CurrentShortcut>,
    mods: Vec<String>,
    key: String,
) -> Result<(), String> {
    // Unregister the currently active shortcut
    {
        let guard = state.0.lock().map_err(|e| e.to_string())?;
        if let Some(ref sc) = *guard {
            let _ = app.global_shortcut().unregister(sc.clone());
        }
    }

    let mut modifiers = Modifiers::empty();
    for m in &mods {
        match m.as_str() {
            "Ctrl" | "Control" => modifiers |= Modifiers::CONTROL,
            "Shift" => modifiers |= Modifiers::SHIFT,
            "Alt" => modifiers |= Modifiers::ALT,
            "Super" | "Meta" | "Win" => modifiers |= Modifiers::SUPER,
            _ => {}
        }
    }

    let code = key_str_to_code(&key)
        .ok_or_else(|| format!("不支持的按键: {key}"))?;

    let new_shortcut = Shortcut::new(
        if modifiers.is_empty() { None } else { Some(modifiers) },
        code,
    );
    let shortcut_for_state = new_shortcut.clone();

    app.global_shortcut()
        .on_shortcut(new_shortcut, |ah, _event, _shortcut| {
            let ah = ah.clone();
            tauri::async_runtime::spawn(async move {
                let _ = ah.emit("trigger-capture", "region");
            });
        })
        .map_err(|e| e.to_string())?;

    *state.0.lock().map_err(|e| e.to_string())? = Some(shortcut_for_state);
    Ok(())
}

fn key_str_to_code(key: &str) -> Option<Code> {
    match key {
        "A" => Some(Code::KeyA), "B" => Some(Code::KeyB), "C" => Some(Code::KeyC),
        "D" => Some(Code::KeyD), "E" => Some(Code::KeyE), "F" => Some(Code::KeyF),
        "G" => Some(Code::KeyG), "H" => Some(Code::KeyH), "I" => Some(Code::KeyI),
        "J" => Some(Code::KeyJ), "K" => Some(Code::KeyK), "L" => Some(Code::KeyL),
        "M" => Some(Code::KeyM), "N" => Some(Code::KeyN), "O" => Some(Code::KeyO),
        "P" => Some(Code::KeyP), "Q" => Some(Code::KeyQ), "R" => Some(Code::KeyR),
        "S" => Some(Code::KeyS), "T" => Some(Code::KeyT), "U" => Some(Code::KeyU),
        "V" => Some(Code::KeyV), "W" => Some(Code::KeyW), "X" => Some(Code::KeyX),
        "Y" => Some(Code::KeyY), "Z" => Some(Code::KeyZ),
        "0" => Some(Code::Digit0), "1" => Some(Code::Digit1), "2" => Some(Code::Digit2),
        "3" => Some(Code::Digit3), "4" => Some(Code::Digit4), "5" => Some(Code::Digit5),
        "6" => Some(Code::Digit6), "7" => Some(Code::Digit7), "8" => Some(Code::Digit8),
        "9" => Some(Code::Digit9),
        "F1"  => Some(Code::F1),  "F2"  => Some(Code::F2),  "F3"  => Some(Code::F3),
        "F4"  => Some(Code::F4),  "F5"  => Some(Code::F5),  "F6"  => Some(Code::F6),
        "F7"  => Some(Code::F7),  "F8"  => Some(Code::F8),  "F9"  => Some(Code::F9),
        "F10" => Some(Code::F10), "F11" => Some(Code::F11), "F12" => Some(Code::F12),
        _ => None,
    }
}
