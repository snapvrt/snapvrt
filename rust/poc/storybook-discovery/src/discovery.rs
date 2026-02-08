use std::collections::HashMap;

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("failed to fetch index.json from {url}")]
    Fetch {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("failed to parse index.json from {url}")]
    Parse {
        url: String,
        #[source]
        source: reqwest::Error,
    },
}

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
pub async fn discover(base_url: &str) -> Result<Vec<Story>, DiscoveryError> {
    let base_url = base_url.trim_end_matches('/');
    let index_url = format!("{base_url}/index.json");

    let response = reqwest::get(&index_url)
        .await
        .map_err(|source| DiscoveryError::Fetch {
            url: index_url.clone(),
            source,
        })?;

    let index: IndexResponse = response
        .json()
        .await
        .map_err(|source| DiscoveryError::Parse {
            url: index_url,
            source,
        })?;

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
