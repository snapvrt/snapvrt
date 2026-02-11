pub mod chrome;
pub mod connection;

pub use self::chrome::Chrome;
pub use self::connection::CdpConnection;

/// Clip region in CSS pixels (used by `Page.captureScreenshot`).
pub struct ClipRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
