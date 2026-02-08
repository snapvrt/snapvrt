use std::time::Instant;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::json;

use crate::cdp::CdpConnection;

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

/// JavaScript to inject a <style> element with the given CSS.
/// The CSS is embedded as a template literal.
const INJECT_CSS_JS_TEMPLATE: &str = r#"
(function() {
    const style = document.createElement('style');
    style.textContent = `CSS_PLACEHOLDER`;
    document.head.appendChild(style);
})()
"#;

/// JavaScript to get the <body> bounding rect.
const GET_BODY_BOUNDS_JS: &str = r#"
(function() {
    const rect = document.body.getBoundingClientRect();
    return JSON.stringify({
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height
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

#[derive(Deserialize)]
struct BodyBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Full capture pipeline: set viewport → navigate → inject CSS → wait ready → get bounds → screenshot.
///
/// Uses only 4 CDP domains: Emulation, Page, Runtime, Target.
/// CSS injection and body bounds use `Runtime.evaluate` (like headless_chrome PoC)
/// instead of the CSS/DOM CDP domains (like chromiumoxide PoC).
pub async fn capture(conn: &mut CdpConnection, req: &CaptureRequest) -> Result<CaptureResult> {
    // 1. Set viewport (per-capture)
    conn.call(
        "Emulation.setDeviceMetricsOverride",
        json!({
            "width": req.width,
            "height": req.height,
            "deviceScaleFactor": req.scale,
            "mobile": false,
        }),
    )
    .await
    .context("Failed to set device metrics")?;

    // 2. Enable page lifecycle events
    conn.call("Page.enable", json!({}))
        .await
        .context("Failed to enable Page domain")?;

    // 3. Navigate
    let nav_start = Instant::now();
    conn.call("Page.navigate", json!({"url": req.url}))
        .await
        .context("Failed to navigate")?;

    // 4. Wait for page load
    conn.wait_event("Page.loadEventFired")
        .await
        .context("Timed out waiting for page load")?;
    let navigate_ms = nav_start.elapsed().as_millis();

    // 5. Inject animation-disabling CSS via Runtime.evaluate
    let inject_css_js =
        INJECT_CSS_JS_TEMPLATE.replace("CSS_PLACEHOLDER", &css_for_template_literal());
    conn.call("Runtime.evaluate", json!({"expression": inject_css_js}))
        .await
        .context("Failed to inject CSS")?;

    // 6. Wait for ready (fonts + DOM stable)
    let ready_start = Instant::now();
    let ready_result = conn
        .call(
            "Runtime.evaluate",
            json!({
                "expression": WAIT_FOR_READY_JS,
                "awaitPromise": true,
            }),
        )
        .await
        .context("Ready detection failed")?;

    // Check for JS exceptions
    if let Some(desc) = ready_result
        .get("exceptionDetails")
        .and_then(|e| e.get("exception"))
        .and_then(|e| e.get("description"))
        .and_then(|d| d.as_str())
    {
        anyhow::bail!("Ready detection JS error: {desc}");
    }
    let ready_ms = ready_start.elapsed().as_millis();

    // 7. Get body bounding rect via Runtime.evaluate
    let bounds_result = conn
        .call(
            "Runtime.evaluate",
            json!({"expression": GET_BODY_BOUNDS_JS, "returnByValue": true}),
        )
        .await
        .context("Failed to get body bounds")?;

    let bounds_json = bounds_result["result"]["value"]
        .as_str()
        .context("Body bounds: no string value returned")?;
    let bounds: BodyBounds =
        serde_json::from_str(bounds_json).context("Failed to parse body bounds JSON")?;

    // 8. Screenshot clipped to body bounds
    let screenshot_start = Instant::now();
    let screenshot_result = conn
        .call(
            "Page.captureScreenshot",
            json!({
                "format": "png",
                "clip": {
                    "x": bounds.x,
                    "y": bounds.y,
                    "width": bounds.width,
                    "height": bounds.height,
                    "scale": req.scale,
                },
            }),
        )
        .await
        .context("Failed to capture screenshot")?;
    let screenshot_ms = screenshot_start.elapsed().as_millis();

    let b64_data = screenshot_result["data"]
        .as_str()
        .context("No screenshot data in response")?;

    use base64::Engine;
    let png = base64::engine::general_purpose::STANDARD
        .decode(b64_data)
        .context("Failed to decode base64 screenshot")?;

    Ok(CaptureResult {
        png,
        body_x: bounds.x,
        body_y: bounds.y,
        body_width: bounds.width,
        body_height: bounds.height,
        timing: TimingMetrics {
            navigate_ms,
            ready_ms,
            screenshot_ms,
        },
    })
}

/// Escape the CSS for safe embedding in a JS template literal.
fn css_for_template_literal() -> String {
    DISABLE_ANIMATIONS_CSS
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${")
}
