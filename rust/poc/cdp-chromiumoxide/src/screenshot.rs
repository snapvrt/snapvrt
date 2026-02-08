use std::time::Instant;

use anyhow::{Context, Result};
use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::css::{CreateStyleSheetParams, SetStyleSheetTextParams};
use chromiumoxide::cdp::browser_protocol::dom::{
    GetBoxModelParams, GetDocumentParams, QuerySelectorParams,
};
use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use chromiumoxide::cdp::browser_protocol::page::{CaptureScreenshotFormat, Viewport};
use chromiumoxide::page::ScreenshotParams;

/// CSS injected to disable animations, transitions, pointer events, and carets.
/// From the snapvrt protocol spec (004-protocols.md).
pub const DISABLE_ANIMATIONS_CSS: &str = r#"
*,
*::before,
*::after {
  transition: none !important;
  animation: none !important;
}
* {
  pointer-events: none !important;
}
* {
  caret-color: transparent !important;
}
"#;

/// JavaScript that waits for page readiness:
/// 1. Fonts loaded (document.fonts.ready)
/// 2. DOM stable (no mutations for 100ms)
/// All with a 10s timeout.
pub const WAIT_FOR_READY_JS: &str = r#"
(function waitForReady() {
    return new Promise((resolve, reject) => {
        const TIMEOUT = 10000;
        const DOM_SETTLE_MS = 100;

        const timer = setTimeout(() => {
            reject(new Error('Ready detection timed out after 10s'));
        }, TIMEOUT);

        const fontsReady = document.fonts.ready;

        const domStable = new Promise((res) => {
            let settleTimer = null;
            const observer = new MutationObserver(() => {
                if (settleTimer) clearTimeout(settleTimer);
                settleTimer = setTimeout(() => {
                    observer.disconnect();
                    res();
                }, DOM_SETTLE_MS);
            });
            observer.observe(document.documentElement, {
                childList: true,
                subtree: true,
                attributes: true,
                characterData: true,
            });
            // If DOM is already stable, resolve after settle period
            settleTimer = setTimeout(() => {
                observer.disconnect();
                res();
            }, DOM_SETTLE_MS);
        });

        Promise.all([fontsReady, domStable]).then(() => {
            clearTimeout(timer);
            resolve('ready');
        }).catch((err) => {
            clearTimeout(timer);
            reject(err);
        });
    });
})()
"#;

/// Parameters for a single capture operation.
pub struct CaptureRequest {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub scale: u32,
}

/// Result of a capture operation — PNG bytes plus metadata.
pub struct CaptureResult {
    pub png: Vec<u8>,
    pub body_x: f64,
    pub body_y: f64,
    pub body_width: f64,
    pub body_height: f64,
    pub timing: TimingMetrics,
}

/// Per-phase timing for a single capture.
pub struct TimingMetrics {
    pub navigate_ms: u128,
    pub ready_ms: u128,
    pub screenshot_ms: u128,
}

/// Full capture pipeline: set viewport → navigate → inject CSS → wait ready → get bounds → screenshot.
///
/// The page must have DOM + CSS domains already enabled (via `ManagedBrowser::new_page()`).
/// Viewport is set here so a tab can be reused with different viewports across captures.
/// Returns `CaptureResult` with PNG bytes — caller decides I/O.
pub async fn capture(page: &Page, req: &CaptureRequest) -> Result<CaptureResult> {
    // --- Set viewport (per-capture, not per-page) ---
    page.execute(
        SetDeviceMetricsOverrideParams::builder()
            .width(req.width as i64)
            .height(req.height as i64)
            .device_scale_factor(req.scale as f64)
            .mobile(false)
            .build()
            .map_err(|e| anyhow::anyhow!("{e}"))?,
    )
    .await
    .context("Failed to set device metrics")?;

    // --- Navigate ---
    let nav_start = Instant::now();
    page.goto(&req.url)
        .await
        .context("Failed to navigate to URL")?;
    let navigate_ms = nav_start.elapsed().as_millis();

    // --- Inject animation-disabling CSS via CDP ---
    let frame_id = page
        .mainframe()
        .await
        .context("Failed to get main frame")?
        .context("No main frame")?;

    let sheet = page
        .execute(CreateStyleSheetParams::new(frame_id))
        .await
        .context("Failed to create stylesheet")?;

    page.execute(SetStyleSheetTextParams::new(
        sheet.result.style_sheet_id,
        DISABLE_ANIMATIONS_CSS,
    ))
    .await
    .context("Failed to set stylesheet text")?;

    // --- Wait for ready (fonts + DOM stable) ---
    let ready_start = Instant::now();
    page.evaluate(WAIT_FOR_READY_JS)
        .await
        .context("Ready detection failed")?;
    let ready_ms = ready_start.elapsed().as_millis();

    // --- Get body bounding box via DOM.getBoxModel ---
    let doc = page
        .execute(GetDocumentParams::default())
        .await
        .context("Failed to get document")?;

    let body = page
        .execute(QuerySelectorParams::new(doc.result.root.node_id, "body"))
        .await
        .context("Failed to query body element")?;

    let box_model = page
        .execute(
            GetBoxModelParams::builder()
                .node_id(body.result.node_id)
                .build(),
        )
        .await
        .context("Failed to get box model")?;

    let border = box_model.result.model.border.inner();
    let (body_x, body_y) = (border[0], border[1]);
    let (body_width, body_height) = (border[2] - border[0], border[5] - border[1]);

    // --- Screenshot clipped to body ---
    let screenshot_start = Instant::now();
    let clip = Viewport {
        x: body_x,
        y: body_y,
        width: body_width,
        height: body_height,
        scale: req.scale as f64,
    };

    let png = page
        .screenshot(
            ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .clip(clip)
                .build(),
        )
        .await
        .context("Failed to capture screenshot")?;
    let screenshot_ms = screenshot_start.elapsed().as_millis();

    Ok(CaptureResult {
        png,
        body_x,
        body_y,
        body_width,
        body_height,
        timing: TimingMetrics {
            navigate_ms,
            ready_ms,
            screenshot_ms,
        },
    })
}
