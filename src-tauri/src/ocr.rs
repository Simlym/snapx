//! On-device text recognition.
//!
//! Windows uses the built-in `Windows.Media.Ocr` engine — fully offline, no
//! bundled model, recognises whatever language packs the user has installed.
//! Other platforms return a clear "unsupported" error for now (macOS Vision
//! is a natural future addition).

/// Recognise text in a base64-encoded PNG. Returns the recognised text joined
/// by newlines (one line per OCR line), or an error describing why it failed.
pub fn recognize_base64_png(image_data: &str) -> Result<String, String> {
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, image_data)
        .map_err(|e| format!("base64 解码失败: {e}"))?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("图片解码失败: {e}"))?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    recognize_rgba(rgba.as_raw(), w, h)
}

#[cfg(windows)]
pub fn recognize_rgba(rgba: &[u8], width: u32, height: u32) -> Result<String, String> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Security::Cryptography::CryptographicBuffer;

    if width == 0 || height == 0 {
        return Err("空图像".into());
    }

    // WinRT's SoftwareBitmap with Bgra8 expects B,G,R,A byte order; our source
    // is R,G,B,A. Swap the R/B channels into an owned buffer.
    let mut bgra = vec![0u8; rgba.len()];
    for (dst, src) in bgra.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
        dst[3] = src[3];
    }

    let buffer = CryptographicBuffer::CreateFromByteArray(&bgra)
        .map_err(|e| format!("创建缓冲区失败: {e}"))?;
    // 4-arg base overload (no alpha mode) — unambiguous in windows-rs bindings;
    // alpha handling is irrelevant for OCR.
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        width as i32,
        height as i32,
    )
    .map_err(|e| format!("创建位图失败: {e}"))?;

    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|e| format!("OCR 引擎初始化失败: {e}"))?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .map_err(|e| format!("OCR 调用失败: {e}"))?
        .get()
        .map_err(|e| format!("OCR 识别失败: {e}"))?;

    // Join recognised lines with newlines; fall back to the flat Text() if the
    // line enumeration is empty.
    let mut out = String::new();
    if let Ok(lines) = result.Lines() {
        for line in lines {
            if let Ok(text) = line.Text() {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&text.to_string());
            }
        }
    }
    if out.is_empty() {
        if let Ok(text) = result.Text() {
            out = text.to_string();
        }
    }
    Ok(out)
}

#[cfg(not(windows))]
pub fn recognize_rgba(_rgba: &[u8], _width: u32, _height: u32) -> Result<String, String> {
    Err("当前平台暂不支持文字识别（OCR）".into())
}
