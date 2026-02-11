use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, Command};
use tracing::{debug, info};

static BROWSER_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Chrome process lifecycle: launch (or connect to remote), create tabs, kill.
pub struct Chrome {
    /// None when connected to a remote Chrome we don't own.
    child: Option<Child>,
    /// host:port for HTTP JSON API and building per-tab WebSocket URLs.
    host_port: String,
    /// Temp data dir, cleaned up on drop (only for local Chrome).
    data_dir: Option<PathBuf>,
}

impl Chrome {
    /// Launch a local Chrome with `--remote-debugging-port=0` (auto-assign).
    /// Parses `DevTools listening on ws://...` from stderr.
    pub async fn launch() -> Result<Self> {
        let id = BROWSER_COUNTER.fetch_add(1, Ordering::Relaxed);
        let data_dir = std::env::temp_dir().join(format!("snapvrt-{}-{id}", std::process::id()));

        let chrome_path = find_chrome()?;
        info!(path = %chrome_path, "launching local Chrome");

        let mut child = Command::new(chrome_path)
            .args([
                "--headless=new",
                "--disable-gpu",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-extensions",
                "--disable-background-networking",
                "--disable-background-timer-throttling",
                "--disable-backgrounding-occluded-windows",
                "--disable-renderer-backgrounding",
                "--disable-ipc-flooding-protection",
                "--disable-sync",
                "--disable-translate",
                "--mute-audio",
                "--hide-scrollbars",
                "--remote-debugging-port=0",
            ])
            .arg(format!("--user-data-dir={}", data_dir.display()))
            .stderr(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stdin(std::process::Stdio::null())
            .spawn()
            .context("Failed to spawn Chrome")?;

        let stderr = child.stderr.take().context("No stderr from Chrome")?;
        let mut lines = tokio::io::BufReader::new(stderr).lines();

        // Read stderr until we find the DevTools listening line.
        let debug_url: String = loop {
            let line: Option<String> =
                tokio::time::timeout(std::time::Duration::from_secs(10), lines.next_line())
                    .await
                    .context("Timed out waiting for Chrome DevTools URL")?
                    .context("Failed to read Chrome stderr")?;

            match line {
                Some(ref text) if text.contains("DevTools listening on ") => {
                    let url = text
                        .split("DevTools listening on ")
                        .nth(1)
                        .context("Failed to parse DevTools URL")?
                        .trim()
                        .to_string();
                    break url;
                }
                Some(_) => continue,
                None => bail!("Chrome exited before printing DevTools URL"),
            }
        };

        debug!(url = %debug_url, "Chrome DevTools URL discovered");
        let host_port = parse_host_port(&debug_url)?;

        Ok(Self {
            child: Some(child),
            host_port,
            data_dir: Some(data_dir),
        })
    }

    /// Connect to a remote Chrome instance (e.g. running in Docker).
    ///
    /// `base_url` is `http://host:port` — we hit `/json/version` to verify
    /// connectivity, then use the HTTP JSON API for tab management.
    pub async fn connect(base_url: &str) -> Result<Self> {
        let base = base_url.trim_end_matches('/');
        let version_url = format!("{base}/json/version");

        // Extract the host:port the user gave us — this is what we'll use for
        // all HTTP and WebSocket connections, regardless of what Chrome reports
        // internally (e.g. Docker container address).
        let caller_host_port = base
            .split("://")
            .nth(1)
            .context("Invalid chrome_url: no scheme")?
            .to_string();

        info!(url = %version_url, "connecting to remote Chrome");
        reqwest::get(&version_url)
            .await
            .with_context(|| format!("Failed to reach Chrome at {version_url}"))?
            .error_for_status()
            .context("Chrome /json/version returned error")?;

        debug!("remote Chrome is reachable");

        Ok(Self {
            child: None,
            host_port: caller_host_port,
            data_dir: None,
        })
    }

    /// Create a new tab via `PUT /json/new` (HTTP JSON API, no browser WS needed).
    /// Returns `(target_id, ws_url)` where `ws_url` is the per-target WebSocket.
    pub async fn create_tab(&self) -> Result<(String, String)> {
        let url = format!("http://{}/json/new?about:blank", self.host_port);
        debug!(url = %url, "PUT /json/new");

        let resp: serde_json::Value = reqwest::Client::new()
            .put(&url)
            .send()
            .await
            .context("PUT /json/new failed")?
            .json()
            .await
            .context("Failed to parse /json/new response")?;

        let target_id = resp["id"]
            .as_str()
            .context("No id in /json/new response")?
            .to_string();

        let ws_url = format!("ws://{}/devtools/page/{target_id}", self.host_port);
        debug!(target_id = %target_id, "tab created");

        Ok((target_id, ws_url))
    }

    /// Close a tab via `GET /json/close/<id>` (HTTP JSON API, no browser WS needed).
    pub async fn close_tab(&self, target_id: &str) -> Result<()> {
        let url = format!("http://{}/json/close/{target_id}", self.host_port);
        reqwest::get(&url)
            .await
            .with_context(|| format!("GET /json/close/{target_id} failed"))?;
        debug!(target_id, "tab closed");
        Ok(())
    }

    /// Kill the Chrome process (no-op for remote connections).
    pub fn kill(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
        }
    }
}

impl Drop for Chrome {
    fn drop(&mut self) {
        self.kill();
        if let Some(ref data_dir) = self.data_dir {
            let _ = std::fs::remove_dir_all(data_dir);
        }
    }
}

/// Extract `host:port` from a WebSocket URL like `ws://127.0.0.1:9222/devtools/browser/...`
fn parse_host_port(ws_url: &str) -> Result<String> {
    let after_scheme = ws_url
        .split("://")
        .nth(1)
        .context("Invalid WebSocket URL: no scheme")?;
    let host_port = after_scheme
        .split('/')
        .next()
        .context("Invalid WebSocket URL: no host:port")?;
    Ok(host_port.to_string())
}

/// Find the Chrome executable on the current platform.
fn find_chrome() -> Result<String> {
    let candidates = if cfg!(target_os = "macos") {
        vec![
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
        ]
    } else {
        vec![
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
        ]
    };

    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return Ok(path.to_string());
        }
    }

    // On Linux, check PATH
    if !cfg!(target_os = "macos") {
        for name in &candidates {
            if std::process::Command::new("which")
                .arg(name)
                .output()
                .is_ok_and(|o| o.status.success())
            {
                return Ok(name.to_string());
            }
        }
    }

    bail!("Chrome not found. Tried: {}", candidates.join(", "))
}
