use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScreenshotKind {
    #[default]
    Stable,
    Single,
}

/// Configuration for the capture pipeline.
///
/// Strategy fields are `Option` — `None` means "use default".
/// Serves both TOML deserialization (`[capture]`) and CLI argument parsing.
#[derive(Clone, Debug, Default, clap::Args, Serialize, Deserialize)]
pub struct CaptureConfig {
    #[arg(long, value_enum)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<ScreenshotKind>,

    #[arg(long)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stability_attempts: Option<u32>,

    #[arg(long)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stability_delay_ms: Option<u64>,

    /// Number of parallel browser tabs for capturing
    #[arg(long, short = 'p')]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel: Option<usize>,

    /// Connect to a remote Chrome instead of launching a local one.
    /// Value is `http://host:port` (e.g. `http://localhost:9222`).
    #[arg(long)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chrome_url: Option<String>,
}

impl CaptureConfig {
    /// Overlay non-None fields from `other` onto self.
    pub fn merge(&mut self, other: &CaptureConfig) {
        if other.screenshot.is_some() {
            self.screenshot = other.screenshot;
        }
        if other.stability_attempts.is_some() {
            self.stability_attempts = other.stability_attempts;
        }
        if other.stability_delay_ms.is_some() {
            self.stability_delay_ms = other.stability_delay_ms;
        }
        if other.parallel.is_some() {
            self.parallel = other.parallel;
        }
        if other.chrome_url.is_some() {
            self.chrome_url = other.chrome_url.clone();
        }
    }

    pub fn parallel(&self) -> usize {
        self.parallel.unwrap_or(4)
    }
}
