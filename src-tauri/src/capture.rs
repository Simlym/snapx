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

/// A window's bounds, translated into monitor-local physical pixels so the
/// frontend can map them onto the captured screenshot for edge detection /
/// snap-to-window selection.
#[derive(Debug, Serialize)]
pub struct WindowInfo {
    pub id: u32,
    pub title: String,
    pub app_name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Encode an RGBA8 buffer to a base64 PNG using fast compression.
/// Fast deflate + no row filtering trades a slightly larger file for a big
/// drop in encode time — the right call for an interactive capture path.
/// Public wrapper around the fast PNG encoder, used by the stitcher to emit
/// the finished long screenshot.
pub fn encode_rgba_png_base64(rgba: &[u8], width: u32, height: u32) -> Result<String, String> {
    encode_rgba_to_base64_png(rgba, width, height)
}

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
    let t_start = std::time::Instant::now();

    let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
    let t_enum = t_start.elapsed();

    let monitor = monitors
        .get(monitor_index)
        .ok_or_else(|| "Monitor not found".to_string())?;

    let raw = monitor.capture_image().map_err(|e| e.to_string())?;
    let t_capture = t_start.elapsed();

    let width = raw.width();
    let height = raw.height();

    let base64_data = encode_rgba_to_base64_png(raw.as_raw(), width, height)?;
    let t_encode = t_start.elapsed();

    eprintln!(
        "[perf] capture_full_monitor: enum={:.0}ms capture={:.0}ms encode={:.0}ms total={:.0}ms ({}x{})",
        t_enum.as_secs_f64() * 1000.0,
        (t_capture - t_enum).as_secs_f64() * 1000.0,
        (t_encode - t_capture).as_secs_f64() * 1000.0,
        t_encode.as_secs_f64() * 1000.0,
        width, height,
    );

    Ok(CaptureResult {
        image_data: base64_data,
        width,
        height,
        monitor_id: monitor_index as u32,
    })
}

/// Enumerate visible windows overlapping a monitor, with bounds expressed in
/// that monitor's local physical pixels (origin at the monitor's top-left).
/// Ordered front-to-back is not guaranteed by xcap; we return them in z-order
/// as reported and let the frontend pick the smallest window under the cursor.
pub fn list_windows_for_monitor(monitor_index: usize) -> Result<Vec<WindowInfo>, String> {
    let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
    let monitor = monitors
        .get(monitor_index)
        .ok_or_else(|| "Monitor not found".to_string())?;
    let (mx, my) = (monitor.x(), monitor.y());
    let (mw, mh) = (monitor.width() as i32, monitor.height() as i32);

    let windows = xcap::Window::all().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for w in windows {
        if w.is_minimized() {
            continue;
        }
        // Skip SnapX's own windows (overlay/pins) so they don't get detected.
        if w.app_name().eq_ignore_ascii_case("snapx")
            || w.title().starts_with("SnapX")
        {
            continue;
        }
        let ww = w.width() as i32;
        let wh = w.height() as i32;
        if ww <= 0 || wh <= 0 {
            continue;
        }
        // Local coordinates relative to the monitor's top-left.
        let lx = w.x() - mx;
        let ly = w.y() - my;
        // Skip windows that don't overlap this monitor at all.
        if lx + ww <= 0 || ly + wh <= 0 || lx >= mw || ly >= mh {
            continue;
        }
        out.push(WindowInfo {
            id: w.id(),
            title: w.title().to_string(),
            app_name: w.app_name().to_string(),
            x: lx,
            y: ly,
            width: w.width(),
            height: w.height(),
        });
    }
    Ok(out)
}

/// Capture a region of a specific monitor
pub fn capture_monitor_region(
    monitor_index: usize,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<CaptureResult, String> {
    let t_start = std::time::Instant::now();

    let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
    let monitor = monitors
        .get(monitor_index)
        .ok_or_else(|| "Monitor not found".to_string())?;

    let raw = monitor.capture_image().map_err(|e| e.to_string())?;
    let t_capture = t_start.elapsed();
    let full_image = image::DynamicImage::ImageRgba8(raw);

    // Crop the region - ensure bounds are valid
    let x = x.min(full_image.width().saturating_sub(1));
    let y = y.min(full_image.height().saturating_sub(1));
    let width = width.min(full_image.width() - x);
    let height = height.min(full_image.height() - y);

    let cropped = full_image.crop_imm(x, y, width, height).to_rgba8();
    let (cw, ch) = (cropped.width(), cropped.height());

    let base64_data = encode_rgba_to_base64_png(cropped.as_raw(), cw, ch)?;
    let t_encode = t_start.elapsed();

    eprintln!(
        "[perf] capture_monitor_region: capture={:.0}ms crop+encode={:.0}ms total={:.0}ms ({}x{})",
        t_capture.as_secs_f64() * 1000.0,
        (t_encode - t_capture).as_secs_f64() * 1000.0,
        t_encode.as_secs_f64() * 1000.0,
        cw, ch,
    );

    Ok(CaptureResult {
        image_data: base64_data,
        width: cw,
        height: ch,
        monitor_id: monitor_index as u32,
    })
}
