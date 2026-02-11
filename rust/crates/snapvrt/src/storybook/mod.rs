pub mod discovery;

pub use self::discovery::Storybook;

/// Normalize a string for filter comparison: lowercase + treat `_` and ` ` as equivalent.
/// This lets users filter by either the raw story fields (spaces) or the
/// snapshot ID shown in the terminal (underscores).
pub(crate) fn normalize_for_filter(s: &str) -> String {
    s.to_lowercase().replace('_', " ")
}

/// A discovered story ready for capture.
#[derive(Debug, Clone)]
pub struct Story {
    pub id: String,
    pub name: String,
    pub title: String,
    pub tags: Vec<String>,
}

impl Story {
    /// Check if this story should be skipped (tagged `snapvrt-skip`).
    pub fn is_skipped(&self) -> bool {
        self.tags.iter().any(|t| t == "snapvrt-skip")
    }

    /// Check if any story field matches a case-insensitive pattern.
    pub fn matches_filter(&self, pattern: &str) -> bool {
        let p = normalize_for_filter(pattern);
        normalize_for_filter(&self.id).contains(&p)
            || normalize_for_filter(&self.title).contains(&p)
            || normalize_for_filter(&self.name).contains(&p)
    }
}
