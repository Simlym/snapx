//! On-device text recognition (OCR).
//!
//! Uses PaddleOCR PP-OCRv5 (mobile, fp16) on the MNN inference backend via the
//! `ocr-rs` crate — fully offline, cross-platform, and strong on mixed
//! Chinese/English. The models are embedded in the binary (`include_bytes!`),
//! so there is nothing to install or download at runtime.
//!
//! The MNN engine holds raw C++ handles and is neither `Send` nor `Sync`, so we
//! confine it to a single long-lived worker thread and talk to it over a
//! channel. Recognition returns per-line text plus bounding boxes (in the
//! recognised image's pixel space) so the frontend can lay a selectable text
//! layer over the screenshot — PDF-style "select the text right off the image".

use serde::Serialize;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};

// Embedded PP-OCRv5 mobile models (fp16 ≈ 10 MB total).
const DET_MODEL: &[u8] = include_bytes!("../models/PP-OCRv5_mobile_det_fp16.mnn");
const REC_MODEL: &[u8] = include_bytes!("../models/PP-OCRv5_mobile_rec_fp16.mnn");
const KEYS: &[u8] = include_bytes!("../models/ppocr_keys_v5.txt");

/// One recognised text line with its bounding box, in the *original* image's
/// pixel coordinates (the upscale used internally is divided back out).
#[derive(Debug, Clone, Serialize)]
pub struct OcrLine {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Full OCR result: the recognised lines plus the source image dimensions, so
/// the frontend can map box coordinates onto the on-screen screenshot.
#[derive(Debug, Clone, Serialize)]
pub struct OcrResult {
    pub lines: Vec<OcrLine>,
    pub img_w: u32,
    pub img_h: u32,
}

/// A unit of work for the OCR thread: a base64 PNG and a channel to reply on.
type Job = (String, Sender<Result<OcrResult, String>>);

/// Lazily-spawned worker that owns the engine for the life of the process.
static WORKER: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();

fn worker() -> &'static Mutex<Sender<Job>> {
    WORKER.get_or_init(|| {
        let (tx, rx) = channel::<Job>();
        std::thread::Builder::new()
            .name("snapx-ocr".into())
            .spawn(move || {
                // Build the engine once, on this thread. If it fails, keep the
                // error and report it for every job rather than panicking.
                let engine = ocr_rs::OcrEngine::from_bytes(DET_MODEL, REC_MODEL, KEYS, None);
                for (image_data, reply) in rx {
                    let res = match engine {
                        Ok(ref eng) => recognize_one(eng, &image_data),
                        Err(ref e) => Err(format!("OCR 引擎初始化失败: {e}")),
                    };
                    let _ = reply.send(res);
                }
            })
            .expect("failed to spawn OCR worker thread");
        Mutex::new(tx)
    })
}

/// Decode → (optionally) upscale → recognise → map boxes back to original px.
fn recognize_one(engine: &ocr_rs::OcrEngine, image_data: &str) -> Result<OcrResult, String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image_data)
        .map_err(|e| format!("base64 解码失败: {e}"))?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("图片解码失败: {e}"))?;
    let (ow, oh) = (img.width(), img.height());
    if ow == 0 || oh == 0 {
        return Err("空图像".into());
    }

    // Small captures (screenshots of body text) recognise much better upscaled.
    // 2× for anything under ~1000px on its longest side; map coords back after.
    let scale: f32 = if ow.max(oh) < 1000 { 2.0 } else { 1.0 };
    let work = if scale != 1.0 {
        img.resize(
            (ow as f32 * scale) as u32,
            (oh as f32 * scale) as u32,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };

    let results = engine
        .recognize(&work)
        .map_err(|e| format!("OCR 识别失败: {e}"))?;

    let mut lines: Vec<OcrLine> = results
        .into_iter()
        .map(|r| {
            // `bbox.rect` is an imageproc Rect: left()/top() are i32, width()/
            // height() are u32. Divide the upscale factor back out.
            let rect = r.bbox.rect;
            OcrLine {
                text: r.text,
                x: rect.left() as f32 / scale,
                y: rect.top() as f32 / scale,
                w: rect.width() as f32 / scale,
                h: rect.height() as f32 / scale,
            }
        })
        .collect();

    // Reading order: top-to-bottom, then left-to-right within a row. The 0.6×
    // height tolerance treats boxes on roughly the same baseline as one row.
    lines.sort_by(|a, b| {
        let row = (a.h.min(b.h)) * 0.6;
        if (a.y - b.y).abs() <= row {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    Ok(OcrResult {
        lines,
        img_w: ow,
        img_h: oh,
    })
}

/// Recognise text in a base64-encoded PNG. Blocks until the worker replies;
/// callers run this on a blocking task so the async runtime isn't stalled.
pub fn recognize_base64_png(image_data: &str) -> Result<OcrResult, String> {
    let (reply_tx, reply_rx) = channel();
    worker()
        .lock()
        .map_err(|e| e.to_string())?
        .send((image_data.to_string(), reply_tx))
        .map_err(|_| "OCR 线程不可用".to_string())?;
    reply_rx
        .recv()
        .map_err(|_| "OCR 线程无响应".to_string())?
}
