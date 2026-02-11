use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tracing::debug;

use super::scripts;
use super::strategy::{self, Screenshot};
use super::timing::CaptureTimings;
use crate::cdp::{CdpConnection, Chrome};
use crate::config::CaptureConfig;

/// Delay after viewport resize to let the page reflow.
const VIEWPORT_RESIZE_SETTLE: Duration = Duration::from_millis(500);

/// Parameters for a single capture operation.
pub struct CaptureRequest {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

/// Result of a capture operation.
pub struct CaptureResult {
    pub png: Vec<u8>,
    pub timings: CaptureTimings,
}

// ---------------------------------------------------------------------------
// CdpRenderer / CdpSession
// ---------------------------------------------------------------------------

/// CDP renderer: owns a Chrome instance and produces `CdpSession`s.
pub struct CdpRenderer {
    chrome: Chrome,
    screenshot: Screenshot,
}

impl CdpRenderer {
    pub async fn launch(config: &CaptureConfig) -> Result<Self> {
        let chrome = match &config.chrome_url {
            Some(url) => Chrome::connect(url)
                .await
                .with_context(|| format!("Failed to connect to remote Chrome at {url}"))?,
            None => Chrome::launch().await.context("Failed to launch Chrome")?,
        };
        let screenshot = Screenshot::from_config(config);
        Ok(Self { chrome, screenshot })
    }

    /// Close a session: drop the WebSocket connection, then close the tab.
    pub async fn close_session(&self, session: CdpSession) -> Result<()> {
        let target_id = session.target_id;
        // Drop the WebSocket connection before closing the tab.
        drop(session.conn);
        self.chrome.close_tab(&target_id).await
    }

    pub async fn new_session(&self) -> Result<CdpSession> {
        let (target_id, ws_url) = self.chrome.create_tab().await?;
        debug!(target_id = %target_id, ws_url = %ws_url, "connecting to tab");
        let mut conn = CdpConnection::connect(&ws_url).await?;
        debug!(target_id = %target_id, "enabling domains");
        conn.enable_domains().await?;
        debug!(target_id = %target_id, "session ready");
        Ok(CdpSession {
            conn,
            screenshot: self.screenshot,
            target_id,
        })
    }
}

/// CDP session: owns a single tab connection.
pub struct CdpSession {
    conn: CdpConnection,
    screenshot: Screenshot,
    target_id: String,
}

impl CdpSession {
    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    /// Full capture pipeline.
    ///
    /// Pipeline stages:
    /// 1. Set viewport
    /// 2. Navigate
    /// 3. Wait load event
    /// 4. Wait for network idle
    /// 5. Disable animations
    /// 6. Wait ready (fonts + DOM)
    /// 7. Wait for story root selector
    /// 8. Get clip bounds
    /// 9. Take screenshot (strategy)
    pub async fn capture(&mut self, req: &CaptureRequest) -> Result<CaptureResult> {
        let conn = &mut self.conn;
        let t0 = Instant::now();

        // 1. Set viewport
        debug!(width = req.width, height = req.height, "1/9 set_viewport");
        conn.set_viewport(req.width, req.height).await?;
        let t1 = Instant::now();

        // 2. Navigate
        debug!(url = %req.url, "2/9 navigate");
        conn.navigate(&req.url).await?;
        let t2 = Instant::now();

        // 3. Wait for page load
        debug!("3/9 wait_page_load");
        conn.wait_page_load().await?;
        let t3 = Instant::now();
        debug!(elapsed_ms = (t3 - t2).as_millis() as u64, "3/9 page loaded");

        // 4. Wait for network idle
        debug!("4/9 network_wait");
        conn.wait_network_idle().await?;
        let t4 = Instant::now();
        debug!(
            elapsed_ms = (t4 - t3).as_millis() as u64,
            "4/9 network idle"
        );

        // 5. Disable animations
        debug!("5/9 disable_animations");
        strategy::disable_animations(conn).await?;
        let t5 = Instant::now();

        // 6. Wait for ready (fonts + DOM stable)
        debug!("6/9 wait_ready");
        conn.eval_async(scripts::WAIT_FOR_READY_JS).await?;
        let t6 = Instant::now();
        debug!(elapsed_ms = (t6 - t5).as_millis() as u64, "6/9 ready");

        // 7. Wait for story root selector (poll until visible with non-zero dimensions)
        debug!("7/9 wait_story_root");
        conn.eval_async(scripts::WAIT_FOR_STORY_ROOT_JS).await?;
        let t7 = Instant::now();
        debug!(
            elapsed_ms = (t7 - t6).as_millis() as u64,
            "7/9 story root present"
        );

        // 8. Get clip bounds
        debug!("8/9 get_clip");
        let mut clip = strategy::get_clip(conn).await?;

        // Clamp clip width to viewport.
        let vp_w = req.width as f64;
        if clip.w > vp_w {
            debug!(
                original_w = clip.w,
                viewport_w = vp_w,
                "clamping clip width to viewport"
            );
            clip.w = vp_w;
        }

        // Ensure minimum dimensions (defensive).
        clip.w = clip.w.max(1.0);
        clip.h = clip.h.max(1.0);

        // Resize viewport for tall content.
        let resized = clip.h > req.height as f64;
        if resized {
            let new_h = clip.h.ceil() as u32;
            debug!(
                original_h = req.height,
                new_h, "resizing viewport for tall content"
            );
            conn.set_viewport(req.width, new_h).await?;
            tokio::time::sleep(VIEWPORT_RESIZE_SETTLE).await;
        }

        let t8 = Instant::now();
        debug!(
            x = clip.x,
            y = clip.y,
            w = clip.w,
            h = clip.h,
            resized,
            "8/9 clip bounds"
        );

        // 9. Take screenshot (strategy)
        debug!("9/9 screenshot");
        let png = self.screenshot.take(conn, &clip).await?;
        let t9 = Instant::now();
        debug!(
            bytes = png.len(),
            elapsed_ms = (t9 - t8).as_millis() as u64,
            "9/9 screenshot done"
        );

        // Restore original viewport if resized.
        if resized {
            conn.set_viewport(req.width, req.height).await?;
        }

        let timings = CaptureTimings {
            viewport: t1 - t0,
            navigate: t2 - t1,
            page_load: t3 - t2,
            network: t4 - t3,
            animation: t5 - t4,
            ready: t6 - t5,
            selector: t7 - t6,
            clip: t8 - t7,
            screenshot: t9 - t8,
            total: t9 - t0,
            compare: Duration::ZERO,
        };

        Ok(CaptureResult { png, timings })
    }
}
