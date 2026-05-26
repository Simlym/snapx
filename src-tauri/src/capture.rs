use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ColorType, ImageEncoder};
use serde::Serialize;

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

/// Encode an RGBA8 buffer to a base64 PNG using fast compression.
/// Fast deflate + no row filtering trades a slightly larger file for a big
/// drop in encode time — the right call for an interactive capture path.
fn encode_rgba_to_base64_png(rgba: &[u8], width: u32, height: u32) -> Result<String, String> {
    let mut bytes = Vec::new();
    let encoder = PngEncoder::new_with_quality(&mut bytes, CompressionType::Fast, FilterType::NoFilter);
    encoder
        .write_image(rgba, width, height, ColorType::Rgba8.into())
        .map_err(|e| e.to_string())?;
    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes))
}

/// Capture all screens and return their info
pub fn get_monitor_list() -> Vec<MonitorInfo> {
    let monitors = xcap::Monitor::all().unwrap_or_default();
    monitors
        .iter()
        .enumerate()
        .map(|(i, m)| MonitorInfo {
            id: i as u32,
            name: m.name().to_string(),
            x: m.x() as i32,
            y: m.y() as i32,
            width: m.width() as u32,
            height: m.height() as u32,
            scale_factor: m.scale_factor() as f64,
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

    let raw = monitor.capture_image().map_err(|e| e.to_string())?;
    let width = raw.width();
    let height = raw.height();

    let base64_data = encode_rgba_to_base64_png(raw.as_raw(), width, height)?;

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

    let raw = monitor.capture_image().map_err(|e| e.to_string())?;
    let full_image = image::DynamicImage::ImageRgba8(raw);

    // Crop the region - ensure bounds are valid
    let x = x.min(full_image.width().saturating_sub(1));
    let y = y.min(full_image.height().saturating_sub(1));
    let width = width.min(full_image.width() - x);
    let height = height.min(full_image.height() - y);

    let cropped = full_image.crop_imm(x, y, width, height).to_rgba8();
    let (cw, ch) = (cropped.width(), cropped.height());

    let base64_data = encode_rgba_to_base64_png(cropped.as_raw(), cw, ch)?;

    Ok(CaptureResult {
        image_data: base64_data,
        width: cw,
        height: ch,
        monitor_id: monitor_index as u32,
    })
}
