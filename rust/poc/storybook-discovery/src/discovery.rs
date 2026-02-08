use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::Deserialize;

/// Raw Storybook index.json response.
#[derive(Deserialize)]
pub struct IndexResponse {
    pub v: u32,
    pub entries: HashMap<String, StoryEntry>,
}

/// A single entry from Storybook's index.json.
#[derive(Deserialize)]
pub struct StoryEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub name: String,
    pub title: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A discovered story ready for capture.
pub struct Story {
    pub id: String,
    pub name: String,
    pub title: String,
    pub url: String,
}

/// Fetch index.json from a Storybook instance and return filtered stories.
///
/// Filters out non-story entries (e.g. docs) and stories tagged with `snapvrt-skip`.
/// Returns stories sorted by id for stable output.
pub async fn discover(base_url: &str) -> Result<Vec<Story>> {
    let base_url = base_url.trim_end_matches('/');
    let index_url = format!("{base_url}/index.json");

    let index: IndexResponse = reqwest::get(&index_url)
        .await
        .context("failed to fetch index.json")?
        .json()
        .await
        .context("failed to parse index.json")?;

    let mut stories: Vec<Story> = index
        .entries
        .into_values()
        .filter(|entry| entry.entry_type == "story" && !entry.tags.contains(&"snapvrt-skip".into()))
        .map(|entry| Story {
            url: format!("{base_url}/iframe.html?id={}", entry.id),
            id: entry.id,
            name: entry.name,
            title: entry.title,
        })
        .collect();

    stories.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(stories)
}
