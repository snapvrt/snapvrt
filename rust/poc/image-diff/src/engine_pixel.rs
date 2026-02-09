use image::{Rgba, RgbaImage};

use crate::{DiffEngine, DiffError, DiffResult};

pub struct PixelEngine {
    pub threshold: f64,
}

impl Default for PixelEngine {
    fn default() -> Self {
        Self { threshold: 0.1 }
    }
}

impl DiffEngine for PixelEngine {
    fn name(&self) -> &str {
        "pixel"
    }

    fn diff(&self, left: &RgbaImage, right: &RgbaImage) -> Result<DiffResult, DiffError> {
        if left.dimensions() != right.dimensions() {
            return Err(DiffError::DimensionMismatch {
                left_w: left.width(),
                left_h: left.height(),
                right_w: right.width(),
                right_h: right.height(),
            });
        }

        let (w, h) = left.dimensions();
        let total_pixels = (w as u64) * (h as u64);
        let mut diff_pixels: u64 = 0;
        let mut diff_image = RgbaImage::new(w, h);

        for y in 0..h {
            for x in 0..w {
                let lp = left.get_pixel(x, y);
                let rp = right.get_pixel(x, y);

                let distance = pixel_distance(lp, rp);

                if distance > self.threshold {
                    diff_pixels += 1;
                    // Red for different pixels
                    diff_image.put_pixel(x, y, Rgba([255, 0, 0, 255]));
                } else {
                    // Dimmed original for matching pixels
                    let Rgba([r, g, b, a]) = *lp;
                    diff_image.put_pixel(x, y, Rgba([r / 4, g / 4, b / 4, a]));
                }
            }
        }

        let score = if total_pixels > 0 {
            diff_pixels as f64 / total_pixels as f64
        } else {
            0.0
        };

        Ok(DiffResult {
            diff_pixels,
            total_pixels,
            score,
            diff_image: Some(diff_image),
        })
    }
}

/// Euclidean distance in RGBA space, normalized to 0.0–1.0.
fn pixel_distance(a: &Rgba<u8>, b: &Rgba<u8>) -> f64 {
    let dr = a[0] as f64 - b[0] as f64;
    let dg = a[1] as f64 - b[1] as f64;
    let db = a[2] as f64 - b[2] as f64;
    let da = a[3] as f64 - b[3] as f64;

    ((dr * dr + dg * dg + db * db + da * da) / 4.0).sqrt() / 255.0
}
