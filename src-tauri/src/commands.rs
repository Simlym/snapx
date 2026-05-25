use crate::capture;
use base64::Engine;
use image::ImageFormat;
use std::io::Cursor;
use tauri::clipboard_manager::Clipboard;
use tauri::AppHandle;

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
    // For MVP, window capture is essentially full monitor capture
    // A proper implementation would use window handles
    let idx = monitor_index.unwrap_or(0);
    capture::capture_full_monitor(idx)
}

#[tauri::command]
pub async fn save_to_clipboard(
    app: AppHandle,
    image_data: String,
) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&image_data)
        .map_err(|e| e.to_string())?;
    
    // Write the PNG image to clipboard
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_image(&bytes).map_err(|e| e.to_string())
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
pub async fn list_monitors() -> Result<Vec<capture::MonitorInfo>, String> {
    Ok(capture::get_monitor_list())
}
