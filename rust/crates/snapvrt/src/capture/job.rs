use crate::storybook::{Story, normalize_for_filter};

/// A single capture job.
#[derive(Clone)]
pub struct CaptureJob {
    /// Source name (e.g. "storybook").
    pub source: String,
    /// The story being captured.
    pub story: Story,
    /// Viewport name (e.g. "desktop", "mobile").
    pub viewport: String,
    /// Full URL to navigate to.
    pub url: String,
    /// Viewport width in CSS pixels.
    pub width: u32,
    /// Viewport height in CSS pixels.
    pub height: u32,
}

impl CaptureJob {
    /// Hierarchical snapshot ID used as a relative path.
    /// Layout: `{source}/{viewport}/{title_path}/{name}`.
    /// Title slashes become directory separators, spaces become underscores.
    pub fn snapshot_id(&self) -> String {
        let title_path = self.story.title.replace(' ', "_");
        let name_part = self.story.name.replace(' ', "_");
        format!("{}/{}/{title_path}/{name_part}", self.source, self.viewport)
    }

    /// Check if this job matches a case-insensitive filter pattern.
    /// Strips `.png` suffix from pattern (user may copy from HTML review page).
    /// Normalizes spaces/underscores so both terminal output and raw story
    /// fields can be used interchangeably in filters.
    pub fn matches_filter(&self, pattern: &str) -> bool {
        let pattern = pattern.strip_suffix(".png").unwrap_or(pattern);
        let p = normalize_for_filter(pattern);
        self.story.matches_filter(pattern)
            || normalize_for_filter(&self.viewport).contains(&p)
            || normalize_for_filter(&self.snapshot_id()).contains(&p)
    }
}
