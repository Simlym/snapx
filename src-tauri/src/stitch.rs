//! Long-screenshot stitching.
//!
//! The frontend captures one frame per manual scroll step and feeds each frame
//! to [`Stitcher::add_frame`]. The stitcher finds the vertical overlap between
//! the accumulated image and the incoming frame (the part the user already saw
//! before scrolling) and appends only the new content below it. All frames are
//! assumed to share the same width.

use image::RgbaImage;

/// Accumulates scroll frames into one tall image.
pub struct Stitcher {
    /// The image built so far. `None` until the first frame is added.
    canvas: Option<RgbaImage>,
}

impl Stitcher {
    pub fn new() -> Self {
        Stitcher { canvas: None }
    }

    /// Add the next scroll frame. Returns the current total height after the add.
    pub fn add_frame(&mut self, frame: &RgbaImage) -> Result<u32, String> {
        match self.canvas.take() {
            None => {
                self.canvas = Some(frame.clone());
                Ok(frame.height())
            }
            Some(canvas) => {
                if canvas.width() != frame.width() {
                    self.canvas = Some(canvas);
                    return Err("帧宽度不一致".into());
                }
                let merged = append_below(&canvas, frame);
                let h = merged.height();
                self.canvas = Some(merged);
                Ok(h)
            }
        }
    }

    /// Finish stitching and return the accumulated image.
    pub fn finish(self) -> Option<RgbaImage> {
        self.canvas
    }
}

impl Default for Stitcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Number of rows at the bottom of `prev` we try to match against the top of
/// `next` when searching for the scroll overlap. Larger = more robust against
/// repetitive content, but slower.
const SEARCH_BAND: u32 = 0; // 0 ⇒ derive from frame height at call time.

/// Find how many rows at the top of `next` overlap the bottom of `prev`, then
/// produce a new image = `prev` followed by the non-overlapping tail of `next`.
fn append_below(prev: &RgbaImage, next: &RgbaImage) -> RgbaImage {
    let w = prev.width();
    let overlap = find_overlap(prev, next);
    let new_rows = next.height().saturating_sub(overlap);

    // No new content (user didn't scroll, or frames identical): keep prev.
    if new_rows == 0 {
        return prev.clone();
    }

    let out_h = prev.height() + new_rows;
    let mut out = RgbaImage::new(w, out_h);

    // Copy prev wholesale.
    for y in 0..prev.height() {
        for x in 0..w {
            out.put_pixel(x, y, *prev.get_pixel(x, y));
        }
    }
    // Append the part of next below the overlap.
    for y in 0..new_rows {
        let src_y = overlap + y;
        let dst_y = prev.height() + y;
        for x in 0..w {
            out.put_pixel(x, dst_y, *next.get_pixel(x, src_y));
        }
    }
    out
}

/// Find the row count `k` such that the top `k` rows of `next` match the
/// bottom `k` rows of `prev`. Returns the best overlap found, or 0 if none.
///
/// We scan candidate overlaps from large to small and accept the first that
/// matches within tolerance — the largest overlap is the true scroll position
/// (smaller "matches" are coincidental sub-alignments of repetitive content).
fn find_overlap(prev: &RgbaImage, next: &RgbaImage) -> u32 {
    let w = prev.width();
    if w == 0 {
        return 0;
    }
    let max_overlap = prev.height().min(next.height());
    // Require at least a few rows so we don't lock onto a 1px coincidence.
    let min_overlap = 8u32.min(max_overlap);
    if min_overlap == 0 {
        return 0;
    }

    let _ = SEARCH_BAND; // reserved for future windowing

    for k in (min_overlap..=max_overlap).rev() {
        if rows_match(prev, next, k) {
            return k;
        }
    }
    0
}

/// Do the bottom `k` rows of `prev` equal the top `k` rows of `next` within a
/// small per-channel tolerance (to absorb subpixel AA / compression noise)?
fn rows_match(prev: &RgbaImage, next: &RgbaImage, k: u32) -> bool {
    let w = prev.width();
    let base = prev.height() - k;
    // Sample a stride of rows for speed on tall frames; check every row when short.
    let stride = if k > 64 { k / 64 } else { 1 };
    const TOL: i32 = 12;
    let mut diff_pixels: u64 = 0;
    let mut total: u64 = 0;
    let mut y = 0;
    while y < k {
        for x in 0..w {
            let p = prev.get_pixel(x, base + y);
            let n = next.get_pixel(x, y);
            let d = (p[0] as i32 - n[0] as i32).abs()
                + (p[1] as i32 - n[1] as i32).abs()
                + (p[2] as i32 - n[2] as i32).abs();
            total += 1;
            if d > TOL * 3 {
                diff_pixels += 1;
            }
        }
        y += stride;
    }
    if total == 0 {
        return false;
    }
    // Accept if ≥98.5% of sampled pixels match.
    (diff_pixels as f64) / (total as f64) < 0.015
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// Build an image where each absolute row `y + offset` has a distinct,
    /// non-monotonic colour (so adjacent rows differ sharply, like real
    /// content). Frames at different offsets share pixel-identical overlap rows.
    fn ramp(width: u32, height: u32, offset: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |_x, y| {
            // Hash the absolute row index so neighbouring rows look unrelated.
            let h = (y + offset).wrapping_mul(2654435761) ^ 0x9e3779b9;
            Rgba([(h & 0xff) as u8, ((h >> 8) & 0xff) as u8, ((h >> 16) & 0xff) as u8, 255])
        })
    }

    #[test]
    fn single_frame_passthrough() {
        let mut s = Stitcher::new();
        let f = ramp(10, 20, 0);
        assert_eq!(s.add_frame(&f).unwrap(), 20);
        let out = s.finish().unwrap();
        assert_eq!(out.dimensions(), (10, 20));
    }

    #[test]
    fn two_frames_overlap_appends_only_new_rows() {
        // frame0 covers rows 0..40, frame1 covers rows 30..70 (10px overlap).
        let f0 = ramp(16, 40, 0);
        let f1 = ramp(16, 40, 30);
        let mut s = Stitcher::new();
        s.add_frame(&f0).unwrap();
        let total = s.add_frame(&f1).unwrap();
        // 40 + (40 - 10 overlap) = 70 rows total.
        assert_eq!(total, 70);
        let out = s.finish().unwrap();
        assert_eq!(out.height(), 70);
        // Stitched row 65 should equal the original content at absolute row 65.
        let expect = ((65u32.wrapping_mul(2654435761) ^ 0x9e3779b9) & 0xff) as u8;
        assert_eq!(out.get_pixel(0, 65)[0], expect);
    }

    #[test]
    fn identical_frame_adds_nothing() {
        let f = ramp(16, 40, 0);
        let mut s = Stitcher::new();
        s.add_frame(&f).unwrap();
        let total = s.add_frame(&f).unwrap();
        assert_eq!(total, 40, "fully overlapping frame must not grow the canvas");
    }

    #[test]
    fn mismatched_width_errors() {
        let mut s = Stitcher::new();
        s.add_frame(&ramp(16, 10, 0)).unwrap();
        assert!(s.add_frame(&ramp(20, 10, 5)).is_err());
    }

    #[test]
    fn no_overlap_full_append() {
        // Two totally different frames (no shared rows) → full append.
        let f0 = RgbaImage::from_pixel(8, 12, Rgba([10, 10, 10, 255]));
        let f1 = RgbaImage::from_pixel(8, 12, Rgba([200, 200, 200, 255]));
        let mut s = Stitcher::new();
        s.add_frame(&f0).unwrap();
        let total = s.add_frame(&f1).unwrap();
        assert_eq!(total, 24);
    }
}
