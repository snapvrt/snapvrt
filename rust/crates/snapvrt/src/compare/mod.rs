pub mod diff;

/// Status of a single snapshot comparison.
pub enum SnapshotStatus {
    Pass,
    Fail {
        diff_pixels: u64,
        score: f64,
        dimension_mismatch: Option<(u32, u32, u32, u32)>,
    },
    New,
    Error(String),
}
