use std::time::Duration;

/// Per-stage timing breakdown for a single snapshot.
pub struct CaptureTimings {
    pub viewport: Duration,
    pub navigate: Duration,
    pub page_load: Duration,
    pub network: Duration,
    pub animation: Duration,
    pub ready: Duration,
    pub selector: Duration,
    pub clip: Duration,
    pub screenshot: Duration,
    pub total: Duration,
    /// Time spent on image comparison. Zero when no reference exists.
    pub compare: Duration,
}

impl CaptureTimings {
    /// All durations set to zero.
    pub fn zero() -> Self {
        Self {
            viewport: Duration::ZERO,
            navigate: Duration::ZERO,
            page_load: Duration::ZERO,
            network: Duration::ZERO,
            animation: Duration::ZERO,
            ready: Duration::ZERO,
            selector: Duration::ZERO,
            clip: Duration::ZERO,
            screenshot: Duration::ZERO,
            total: Duration::ZERO,
            compare: Duration::ZERO,
        }
    }
}
