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
