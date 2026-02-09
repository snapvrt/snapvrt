use image::RgbaImage;

use crate::{DiffEngine, DiffError, DiffResult};

pub struct SsimEngine;

impl DiffEngine for SsimEngine {
    fn name(&self) -> &str {
        "ssim"
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

        let result = image_compare::rgba_hybrid_compare(left, right).map_err(|e| match e {
            image_compare::CompareError::DimensionsDiffer => DiffError::DimensionMismatch {
                left_w: left.width(),
                left_h: left.height(),
                right_w: right.width(),
                right_h: right.height(),
            },
            image_compare::CompareError::CalculationFailed(msg) => DiffError::Engine(msg),
        })?;

        // rgba_hybrid_compare score: 1.0 = identical, 0.0 = max difference (SSIM convention).
        // Invert to match our convention: 0.0 = identical, 1.0 = max difference.
        let score = 1.0 - result.score;

        // Convert similarity map to a visual color-mapped diff image.
        let color_map = result.image.to_color_map();
        let diff_image = color_map.to_rgba8();

        // Estimate diff pixel count from the score.
        let diff_pixels = (score * total_pixels as f64).round() as u64;

        Ok(DiffResult {
            diff_pixels,
            total_pixels,
            score,
            diff_image: Some(diff_image),
        })
    }
}
