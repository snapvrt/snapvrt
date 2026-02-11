use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::debug;

use super::Story;

#[derive(Deserialize)]
struct IndexResponse {
    #[allow(dead_code)]
    pub v: u32,
    pub entries: HashMap<String, StoryEntry>,
}

#[derive(Deserialize)]
struct StoryEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub name: String,
    pub title: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl From<StoryEntry> for Story {
    fn from(entry: StoryEntry) -> Self {
        Self {
            id: entry.id,
            name: entry.name,
            title: entry.title,
            tags: entry.tags,
        }
    }
}

/// A Storybook instance at a known URL.
pub struct Storybook {
    base_url: String,
}

impl Storybook {
    /// Connect to a Storybook instance.
    ///
    /// When `local` is false (Docker mode), rewrites `localhost` / `127.0.0.1`
    /// to the host's LAN IP so Chrome in a container can reach Storybook.
    /// Fails fast if the host IP cannot be detected.
    pub fn new(base_url: &str, local: bool) -> Result<Self> {
        let url = if local {
            base_url.to_string()
        } else {
            rewrite_localhost(base_url)?
        };
        Ok(Self {
            base_url: url.trim_end_matches('/').to_string(),
        })
    }

    pub fn url(&self) -> &str {
        &self.base_url
    }

    /// Build the iframe URL for a given story.
    pub fn story_url(&self, story: &Story) -> String {
        format!("{}/iframe.html?id={}", self.base_url, story.id)
    }

    /// Fetch index.json and return all stories.
    ///
    /// Filters out non-story entries (e.g. docs).
    /// Returns stories sorted by id for stable output.
    pub async fn discover(&self) -> Result<Vec<Story>> {
        let index_url = format!("{}/index.json", self.base_url);

        let response = reqwest::get(&index_url)
            .await
            .with_context(|| format!("Failed to fetch {index_url}"))?;

        let index: IndexResponse = response
            .json()
            .await
            .with_context(|| format!("Failed to parse {index_url}"))?;

        let mut stories: Vec<Story> = index
            .entries
            .into_values()
            .filter(|entry| entry.entry_type == "story")
            .map(Story::from)
            .collect();

        stories.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(stories)
    }
}

// ---------------------------------------------------------------------------
// Docker localhost rewriting
// ---------------------------------------------------------------------------

/// Replace `localhost` or `127.0.0.1` with the host's real LAN IP so that a
/// remote Chrome running in Docker can reach services on the host machine.
///
/// Fails if the URL points to localhost but the host IP cannot be detected —
/// Chrome in Docker would not be able to reach Storybook anyway.
fn is_localhost_url(url: &str) -> bool {
    for host in ["localhost", "127.0.0.1"] {
        if let Some(rest) = url.split("://").nth(1) {
            let authority = rest.split('/').next().unwrap_or(rest);
            let hostname = authority.split(':').next().unwrap_or(authority);
            if hostname == host {
                return true;
            }
        }
    }
    false
}

fn rewrite_localhost(url: &str) -> Result<String> {
    if !is_localhost_url(url) {
        return Ok(url.to_string());
    }

    let ip = get_local_ip().context(
        "Cannot detect host IP. Chrome in Docker cannot reach localhost.\n\
         Either use a reachable hostname in storybook.url or run with --local.",
    )?;
    debug!("detected host IP for Docker URL rewrite: {ip}");

    Ok(url
        .replace("://localhost", &format!("://{ip}"))
        .replace("://127.0.0.1", &format!("://{ip}")))
}

/// Detect the host's local IP address using the UDP socket trick.
///
/// Binds a UDP socket and "connects" to a public address (no data is sent —
/// UDP `connect` is a local routing table lookup). Returns the IP the OS
/// chose as the source, which is the host's LAN address.
fn get_local_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    if ip.is_loopback() {
        return None;
    }
    Some(ip)
}
