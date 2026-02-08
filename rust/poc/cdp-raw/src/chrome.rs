use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, Command};

use crate::cdp::CdpConnection;

static BROWSER_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Chrome process lifecycle: launch, create tabs, kill.
pub struct Chrome {
    child: Child,
    debug_url: String,
    port: u16,
}

impl Chrome {
    /// Launch Chrome with `--remote-debugging-port=0` (auto-assign).
    /// Parses `DevTools listening on ws://...` from stderr.
    pub async fn launch() -> Result<Self> {
        let id = BROWSER_COUNTER.fetch_add(1, Ordering::Relaxed);
        let data_dir = std::env::temp_dir().join(format!("cdp-raw-{}-{id}", std::process::id()));

        let chrome_path = find_chrome()?;

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

        // Parse port from URL: ws://127.0.0.1:PORT/devtools/browser/UUID
        let port: u16 = debug_url
            .split("://")
            .nth(1)
            .and_then(|s: &str| s.split(':').nth(1))
            .and_then(|s: &str| s.split('/').next())
            .and_then(|s: &str| s.parse::<u16>().ok())
            .context("Failed to parse port from DevTools URL")?;

        Ok(Self {
            child,
            debug_url,
            port,
        })
    }

    /// Create a new tab via `Target.createTarget` on the browser WebSocket.
    /// Returns `(target_id, ws_url)` where `ws_url` is the per-target WebSocket.
    pub async fn create_tab(&self) -> Result<(String, String)> {
        let mut conn = CdpConnection::connect(&self.debug_url).await?;

        let result = conn
            .call("Target.createTarget", json!({"url": "about:blank"}))
            .await
            .context("Target.createTarget failed")?;

        conn.close().await.ok();

        let target_id = result["targetId"]
            .as_str()
            .context("No targetId in createTarget response")?
            .to_string();

        let ws_url = format!("ws://127.0.0.1:{}/devtools/page/{target_id}", self.port);

        Ok((target_id, ws_url))
    }

    /// Kill the Chrome process.
    pub fn kill(&mut self) {
        let _ = self.child.start_kill();
    }
}

impl Drop for Chrome {
    fn drop(&mut self) {
        self.kill();
    }
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
