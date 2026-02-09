use image::RgbaImage;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiffError {
    #[error("dimension mismatch: {left_w}x{left_h} vs {right_w}x{right_h}")]
    DimensionMismatch {
        left_w: u32,
        left_h: u32,
        right_w: u32,
        right_h: u32,
    },

    #[error("engine error: {0}")]
    Engine(String),
}

pub struct DiffResult {
    /// Number of pixels that differ above the threshold.
    pub diff_pixels: u64,
    /// Total number of pixels in the image.
    pub total_pixels: u64,
    /// 0.0 = identical, 1.0 = completely different.
    pub score: f64,
    /// Visual diff image (if produced by the engine).
    pub diff_image: Option<RgbaImage>,
}

pub trait DiffEngine {
    fn name(&self) -> &str;
    fn diff(&self, left: &RgbaImage, right: &RgbaImage) -> Result<DiffResult, DiffError>;
}

pub mod engine_dify;
pub mod engine_pixel;
pub mod engine_ssim;
