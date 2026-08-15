pub mod discovery;

pub use self::discovery::Storybook;

/// Normalize a string for filter comparison: lowercase + treat `_` and ` ` as equivalent.
/// This lets users filter by either the raw story fields (spaces) or the
/// snapshot ID shown in the terminal (underscores).
pub(crate) fn normalize_for_filter(s: &str) -> String {
    s.to_lowercase().replace('_', " ")
}

/// Whether a stored snapshot name matches a `-f` pattern.
///
/// Shared by the commands that filter snapshots already on disk — `approve` and
/// `review` — so one pattern selects the same set whichever reads it. The `.png`
/// suffix is stripped from the pattern because the HTML report lists file names
/// and they get pasted straight back into a filter.
pub(crate) fn snapshot_name_matches(name: &str, pattern: &str) -> bool {
    let pattern = pattern.strip_suffix(".png").unwrap_or(pattern);
    normalize_for_filter(name).contains(&normalize_for_filter(pattern))
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
