use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::scripts;
use crate::cdp::{CdpConnection, ClipRect};
use crate::config::capture::{CaptureConfig, ScreenshotKind};

// ---------------------------------------------------------------------------
// disable_animations
// ---------------------------------------------------------------------------

/// Disable CSS animations/transitions and finish Web Animations API animations.
pub async fn disable_animations(conn: &mut CdpConnection) -> Result<()> {
    let inject_css_js =
        scripts::INJECT_CSS_JS_TEMPLATE.replace("CSS_PLACEHOLDER", &css_for_template_literal());
    conn.eval(&inject_css_js).await?;
    conn.eval(scripts::FINISH_ANIMATIONS_JS).await?;
    Ok(())
}

/// Escape the CSS for safe embedding in a JS template literal.
fn css_for_template_literal() -> String {
    scripts::DISABLE_ANIMATIONS_CSS
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${")
}

// ---------------------------------------------------------------------------
// get_clip
// ---------------------------------------------------------------------------

/// Get the clip region by walking visible children of the Storybook root.
pub async fn get_clip(conn: &mut CdpConnection) -> Result<ClipRect> {
    let result = conn.eval(scripts::GET_STORY_ROOT_BOUNDS_JS).await?;
    parse_bounds_result(&result)
}

#[derive(Deserialize)]
struct ClipBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn parse_bounds_result(result: &serde_json::Value) -> Result<ClipRect> {
    let json_str = result["result"]["value"]
        .as_str()
        .context("Clip bounds: no string value returned")?;
    let bounds: ClipBounds =
        serde_json::from_str(json_str).context("Failed to parse clip bounds JSON")?;

    Ok(ClipRect {
        x: bounds.x,
        y: bounds.y,
        w: bounds.width,
        h: bounds.height,
    })
}

// ---------------------------------------------------------------------------
// Screenshot
// ---------------------------------------------------------------------------

/// How the final screenshot is taken.
#[derive(Clone, Copy)]
pub enum Screenshot {
    /// Take up to N screenshots, returning when two consecutive are byte-identical.
    Stable { max_attempts: u32, delay: Duration },
    /// Single screenshot, no stability check.
    Single,
}

impl Screenshot {
    pub fn from_config(config: &CaptureConfig) -> Self {
        let kind = config.screenshot.unwrap_or_default();
        let attempts = config.stability_attempts.unwrap_or(3);
        let delay_ms = config.stability_delay_ms.unwrap_or(100);

        match kind {
            ScreenshotKind::Stable => Self::Stable {
                max_attempts: attempts,
                delay: Duration::from_millis(delay_ms),
            },
            ScreenshotKind::Single => Self::Single,
        }
    }

    pub async fn take(&self, conn: &mut CdpConnection, clip: &ClipRect) -> Result<Vec<u8>> {
        match *self {
            Self::Stable {
                max_attempts,
                delay,
            } => {
                let mut prev = conn.capture_screenshot(clip).await?;
                for _ in 1..max_attempts {
                    tokio::time::sleep(delay).await;
                    let curr = conn.capture_screenshot(clip).await?;
                    if curr == prev {
                        return Ok(curr);
                    }
                    prev = curr;
                }
                Ok(prev)
            }
            Self::Single => conn.capture_screenshot(clip).await,
        }
    }
}
