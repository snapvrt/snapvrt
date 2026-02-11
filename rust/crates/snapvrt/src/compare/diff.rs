use anyhow::{Context, Result};
use image::RgbaImage;

/// Maximum possible delta in YIQ color space (used by dify internally).
const MAX_YIQ_POSSIBLE_DELTA: f32 = 35215.0;

/// Pre-computed threshold: MAX_YIQ_POSSIBLE_DELTA * 0.1 * 0.1
const THRESHOLD: f32 = MAX_YIQ_POSSIBLE_DELTA * 0.1 * 0.1;

pub struct CompareResult {
    pub is_match: bool,
    pub diff_pixels: u64,
    #[allow(dead_code)]
    pub total_pixels: u64,
    pub score: f64,
    pub diff_image: Option<RgbaImage>,
    /// `Some((ref_w, ref_h, cur_w, cur_h))` when images have different dimensions.
    pub dimension_mismatch: Option<(u32, u32, u32, u32)>,
}

/// Two-phase comparison:
/// 1. Byte-identical check (memcmp)
/// 2. Perceptual diff via dify
///
/// Runs synchronously — call via `spawn_blocking`.
pub fn compare(reference_png: &[u8], current_png: &[u8]) -> Result<CompareResult> {
    // Phase 1: byte-identical
    if reference_png == current_png {
        return Ok(CompareResult {
            is_match: true,
            diff_pixels: 0,
            total_pixels: 0,
            score: 0.0,
            diff_image: None,
            dimension_mismatch: None,
        });
    }

    // Phase 2: decode and diff
    let left = image::load_from_memory(reference_png)
        .context("Failed to decode reference PNG")?
        .to_rgba8();

    let right = image::load_from_memory(current_png)
        .context("Failed to decode current PNG")?
        .to_rgba8();

    let dimension_mismatch = if left.dimensions() != right.dimensions() {
        Some((left.width(), left.height(), right.width(), right.height()))
    } else {
        None
    };

    // Pad both images to the same canvas size if dimensions differ.
    // Fill colour is magenta (#FF00FF) so the size delta is obvious in the diff overlay.
    let (left, right) = if dimension_mismatch.is_some() {
        let max_w = left.width().max(right.width());
        let max_h = left.height().max(right.height());
        (pad_to(&left, max_w, max_h), pad_to(&right, max_w, max_h))
    } else {
        (left, right)
    };

    let total_pixels = (left.width() as u64) * (left.height() as u64);

    let output_base = Some(dify::cli::OutputImageBase::LeftImage);
    let block_out: Option<std::collections::HashSet<(u32, u32)>> = None;

    match dify::diff::get_results(
        left,
        right,
        THRESHOLD,
        true, // detect anti-aliased
        Some(0.1),
        &output_base,
        &block_out,
    ) {
        Some((diff_count, diff_image)) => {
            let diff_pixels = diff_count.max(0) as u64;
            let score = if total_pixels > 0 {
                diff_pixels as f64 / total_pixels as f64
            } else {
                0.0
            };
            Ok(CompareResult {
                is_match: diff_pixels == 0,
                diff_pixels,
                total_pixels,
                score,
                diff_image: Some(diff_image),
                dimension_mismatch,
            })
        }
        None => Ok(CompareResult {
            is_match: true,
            diff_pixels: 0,
            total_pixels,
            score: 0.0,
            diff_image: None,
            dimension_mismatch,
        }),
    }
}

/// Paste `src` onto a magenta canvas of `w x h`, anchored at top-left.
fn pad_to(src: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    let mut canvas = RgbaImage::from_pixel(w, h, image::Rgba([255, 0, 255, 255]));
    image::imageops::overlay(&mut canvas, src, 0, 0);
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// Create a small test PNG with a solid fill.
    fn solid_png(w: u32, h: u32, color: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, color);
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    /// Decode, flip `n` scattered pixels to red, re-encode.
    fn with_pixel_diffs(png: &[u8], n: u32) -> Vec<u8> {
        let mut img = image::load_from_memory(png).unwrap().to_rgba8();
        let (w, h) = img.dimensions();
        for i in 0..n {
            let x = ((i as u64 * 7919) % w as u64) as u32;
            let y = ((i as u64 * 6271) % h as u64) as u32;
            img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        }
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    // -- memcmp fast path --

    #[test]
    fn identical_bytes_skip_dify() {
        let png = solid_png(100, 100, Rgba([200, 200, 200, 255]));
        let r = compare(&png, &png).unwrap();
        assert!(r.is_match);
        assert_eq!(r.diff_pixels, 0);
        assert_eq!(r.total_pixels, 0); // memcmp path sets 0
        assert!(r.diff_image.is_none());
        assert!(r.dimension_mismatch.is_none());
    }

    // -- dify phase --

    #[test]
    fn pixel_diffs_detected() {
        let reference = solid_png(100, 100, Rgba([200, 200, 200, 255]));
        let current = with_pixel_diffs(&reference, 50);
        let r = compare(&reference, &current).unwrap();
        assert!(!r.is_match);
        assert!(r.diff_pixels > 0);
        assert!(r.score > 0.0);
        assert!(r.diff_image.is_some());
        assert!(r.dimension_mismatch.is_none());
    }

    #[test]
    fn perceptually_identical_is_match() {
        // Two different PNG encodings of the same visual content.
        // Bytes differ (different encoder settings), but dify reports no diff.
        let a = solid_png(50, 50, Rgba([128, 128, 128, 255]));
        let mut img = image::load_from_memory(&a).unwrap().to_rgba8();
        // Nudge one pixel by 1 — below YIQ threshold.
        img.put_pixel(0, 0, Rgba([129, 128, 128, 255]));
        let mut b = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut b), image::ImageFormat::Png)
            .unwrap();
        let r = compare(&a, &b).unwrap();
        // dify should detect 0 diff pixels (below threshold).
        assert_eq!(r.diff_pixels, 0);
    }

    // -- score calculation --

    #[test]
    fn score_is_ratio_of_diff_to_total() {
        let reference = solid_png(100, 100, Rgba([200, 200, 200, 255]));
        let current = with_pixel_diffs(&reference, 20);
        let r = compare(&reference, &current).unwrap();
        let expected = r.diff_pixels as f64 / r.total_pixels as f64;
        assert!((r.score - expected).abs() < 1e-9);
    }

    #[test]
    fn zero_diff_score_is_zero() {
        let a = solid_png(50, 50, Rgba([128, 128, 128, 255]));
        let b = solid_png(50, 50, Rgba([128, 128, 128, 255]));
        // Bytes differ (separate encoding) but pixels are identical.
        let r = compare(&a, &b).unwrap();
        assert_eq!(r.score, 0.0);
    }

    // -- dimension mismatch + padding --

    #[test]
    fn dimension_mismatch_detected() {
        let a = solid_png(100, 100, Rgba([200, 200, 200, 255]));
        let b = solid_png(100, 120, Rgba([200, 200, 200, 255]));
        let r = compare(&a, &b).unwrap();
        assert_eq!(r.dimension_mismatch, Some((100, 100, 100, 120)));
    }

    #[test]
    fn dimension_mismatch_pads_with_magenta() {
        let a = solid_png(10, 10, Rgba([200, 200, 200, 255]));
        let b = solid_png(10, 12, Rgba([200, 200, 200, 255]));
        let r = compare(&a, &b).unwrap();
        // The 2-row padding area (magenta vs grey) produces diff pixels.
        assert!(r.diff_pixels > 0, "padding should cause diff pixels");
        // Total canvas is 10x12 = 120 pixels.
        assert_eq!(r.total_pixels, 120);
    }

    #[test]
    fn width_mismatch_reported() {
        let a = solid_png(100, 50, Rgba([200, 200, 200, 255]));
        let b = solid_png(110, 50, Rgba([200, 200, 200, 255]));
        let r = compare(&a, &b).unwrap();
        assert_eq!(r.dimension_mismatch, Some((100, 50, 110, 50)));
        assert!(r.diff_pixels > 0);
    }
}
