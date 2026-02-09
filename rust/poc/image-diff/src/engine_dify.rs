use image::RgbaImage;

use crate::{DiffEngine, DiffError, DiffResult};

pub struct DifyEngine {
    pub threshold: f32,
    pub detect_anti_aliased: bool,
}

impl Default for DifyEngine {
    fn default() -> Self {
        Self {
            threshold: 0.1,
            detect_anti_aliased: true,
        }
    }
}

/// Maximum possible delta in YIQ color space (used by dify internally).
const MAX_YIQ_POSSIBLE_DELTA: f32 = 35215.0;

impl DiffEngine for DifyEngine {
    fn name(&self) -> &str {
        "dify"
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

        let total_pixels = (left.width() as u64) * (left.height() as u64);

        // dify::diff::get_results takes ownership, so we clone.
        let left_owned = left.clone();
        let right_owned = right.clone();

        // get_results expects a pre-computed threshold:
        // raw_threshold^2 * MAX_YIQ_POSSIBLE_DELTA
        let computed_threshold = MAX_YIQ_POSSIBLE_DELTA * self.threshold * self.threshold;

        let output_base = Some(dify::cli::OutputImageBase::LeftImage);
        let block_out: Option<std::collections::HashSet<(u32, u32)>> = None;

        match dify::diff::get_results(
            left_owned,
            right_owned,
            computed_threshold,
            self.detect_anti_aliased,
            Some(0.1), // blend unchanged pixels dimly so the diff image isn't blank
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
                Ok(DiffResult {
                    diff_pixels,
                    total_pixels,
                    score,
                    diff_image: Some(diff_image),
                })
            }
            None => {
                // None means images are identical
                Ok(DiffResult {
                    diff_pixels: 0,
                    total_pixels,
                    score: 0.0,
                    diff_image: None,
                })
            }
        }
    }
}
