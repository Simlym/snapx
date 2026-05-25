use image::ImageFormat;
use serde::Serialize;
use std::io::Cursor;

/// Monitor info returned to frontend
#[derive(Debug, Serialize)]
pub struct MonitorInfo {
    pub id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub is_primary: bool,
}

/// Captured screenshot data
#[derive(Debug, Serialize)]
pub struct CaptureResult {
    pub image_data: String, // base64 encoded PNG
    pub width: u32,
    pub height: u32,
    pub monitor_id: u32,
}

/// Capture all screens and return their info
pub fn get_monitor_list() -> Vec<MonitorInfo> {
    let monitors = xcap::Monitor::all().unwrap_or_default();
    monitors
        .iter()
        .enumerate()
        .map(|(i, m)| MonitorInfo {
            id: i as u32,
            name: m.name().unwrap_or_default(),
            x: m.x() as i32,
            y: m.y() as i32,
            width: m.width() as u32,
            height: m.height() as u32,
            scale_factor: m.scale_factor(),
            is_primary: m.is_primary(),
        })
        .collect()
}

/// Capture a specific monitor entirely
pub fn capture_full_monitor(monitor_index: usize) -> Result<CaptureResult, String> {
    let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
    let monitor = monitors
        .get(monitor_index)
        .ok_or_else(|| "Monitor not found".to_string())?;

    let image = monitor.capture_image().map_err(|e| e.to_string())?;
    let width = image.width();
    let height = image.height();

    let mut buf = Cursor::new(Vec::new());
    image
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    let base64_data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, buf.into_inner());

    Ok(CaptureResult {
        image_data: base64_data,
        width,
        height,
        monitor_id: monitor_index as u32,
    })
}

/// Capture a region of a specific monitor
pub fn capture_monitor_region(
    monitor_index: usize,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<CaptureResult, String> {
    let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
    let monitor = monitors
        .get(monitor_index)
        .ok_or_else(|| "Monitor not found".to_string())?;

    let full_image = monitor.capture_image().map_err(|e| e.to_string())?;

    // Crop the region - ensure bounds are valid
    let x = x.min(full_image.width().saturating_sub(1));
    let y = y.min(full_image.height().saturating_sub(1));
    let width = width.min(full_image.width() - x);
    let height = height.min(full_image.height() - y);

    let cropped = full_image.crop_imm(x, y, width, height);

    let mut buf = Cursor::new(Vec::new());
    cropped
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    let base64_data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, buf.into_inner());

    Ok(CaptureResult {
        image_data: base64_data,
        width,
        height,
        monitor_id: monitor_index as u32,
    })
}
