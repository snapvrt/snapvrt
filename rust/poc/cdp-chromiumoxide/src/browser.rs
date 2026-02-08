use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use chromiumoxide::Page;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::css::EnableParams as CssEnableParams;
use chromiumoxide::cdp::browser_protocol::dom::EnableParams as DomEnableParams;
use futures::StreamExt;
use tokio::task::{AbortHandle, JoinHandle};

static BROWSER_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Browser lifecycle wrapper that owns the handler task and provides graceful shutdown.
pub struct ManagedBrowser {
    browser: Browser,
    handler_handle: JoinHandle<()>,
    handler_abort: AbortHandle,
}

impl ManagedBrowser {
    /// Launch Chrome and spawn the CDP handler task.
    ///
    /// No viewport args — browser window size is irrelevant since viewport
    /// is set per-capture via `SetDeviceMetricsOverride`.
    pub async fn launch() -> Result<Self> {
        // Each browser instance needs its own user data dir to avoid
        // Chrome's SingletonLock conflict when running multiple instances.
        let id = BROWSER_COUNTER.fetch_add(1, Ordering::Relaxed);
        let data_dir = std::env::temp_dir().join(format!("chromiumoxide-{}-{id}", std::process::id()));

        let (browser, mut handler) = Browser::launch(
            BrowserConfig::builder()
                .user_data_dir(data_dir)
                .build()
                .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .await
        .context("Failed to launch Chrome")?;

        let handler_handle = tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }
        });
        let handler_abort = handler_handle.abort_handle();

        Ok(Self {
            browser,
            handler_handle,
            handler_abort,
        })
    }

    /// Create a blank page with DOM + CSS domains enabled (one-time setup per tab).
    ///
    /// Viewport is NOT set here — it's per-capture in `screenshot::capture()`,
    /// so a tab can be reused across different viewport sizes.
    pub async fn new_page(&self) -> Result<Page> {
        let page = self
            .browser
            .new_page("about:blank")
            .await
            .context("Failed to create page")?;

        page.execute(DomEnableParams::default())
            .await
            .context("Failed to enable DOM domain")?;

        page.execute(CssEnableParams {})
            .await
            .context("Failed to enable CSS domain")?;

        Ok(page)
    }

    /// Graceful shutdown: close browser, then abort handler if still running.
    pub async fn close(mut self) -> Result<()> {
        // Try graceful close first
        let close_result = self.browser.close().await;

        // Abort the handler task regardless — prevents leaked Chrome processes
        // even if the graceful close failed or hung
        self.handler_abort.abort();
        let _ = self.handler_handle.await;

        close_result.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }
}
