use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use headless_chrome::protocol::cdp::Page;
use headless_chrome::{Browser, LaunchOptions, Tab};

/// PoC: CDP screenshot capture via headless Chrome.
///
/// Captures a PNG screenshot of a URL, cropped to the <body> bounding box.
/// Injects animation-disabling CSS and waits for fonts + DOM stability.
#[derive(Parser)]
#[command(name = "poc-cdp-headless-chrome")]
struct Cli {
    /// URL to screenshot
    #[arg(long)]
    url: Option<String>,

    /// Output file path
    #[arg(long, default_value = "screenshot.png")]
    output: PathBuf,

    /// Viewport width in CSS pixels
    #[arg(long, default_value_t = 1366)]
    width: u32,

    /// Viewport height in CSS pixels
    #[arg(long, default_value_t = 768)]
    height: u32,

    /// Device scale factor (1 = standard, 2 = retina)
    #[arg(long, default_value_t = 1)]
    scale: u32,

    /// Parallel mode: capture all example stories using N concurrent tabs in one browser
    #[arg(long)]
    parallel: Option<usize>,
}

const STORYBOOK_BASE: &str = "http://localhost:6006/iframe.html?id=";

const EXAMPLE_STORIES: &[&str] = &[
    "example-button--primary",
    "example-button--secondary",
    "example-button--large",
    "example-button--small",
    "example-header--logged-in",
    "example-header--logged-out",
    "example-page--logged-out",
    "example-page--logged-in",
];

/// CSS injected to disable animations, transitions, pointer events, and carets.
/// From the snapvrt protocol spec (004-protocols.md).
const DISABLE_ANIMATIONS_CSS: &str = r#"
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
const WAIT_FOR_READY_JS: &str = r#"
function waitForReady() {
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
}
waitForReady();
"#;

/// JavaScript to get the <body> bounding rect as JSON.
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(n) = cli.parallel {
        run_parallel(n, cli.width, cli.height, cli.scale)
    } else if let Some(url) = &cli.url {
        run_single(url, &cli.output, cli.width, cli.height, cli.scale)
    } else {
        anyhow::bail!("Specify one of: --url <URL> or --parallel <N>");
    }
}

/// Capture a single story on a tab. Returns PNG bytes + timing + bounds.
fn capture_story(
    tab: &Arc<Tab>,
    url: &str,
    width: u32,
    height: u32,
    scale: u32,
) -> Result<CaptureResult> {
    // Set viewport
    tab.call_method(
        headless_chrome::protocol::cdp::Emulation::SetDeviceMetricsOverride {
            width,
            height,
            device_scale_factor: scale as f64,
            mobile: false,
            scale: None,
            screen_width: None,
            screen_height: None,
            position_x: None,
            position_y: None,
            dont_set_visible_size: None,
            screen_orientation: None,
            viewport: None,
            display_feature: None,
            device_posture: None,
        },
    )
    .context("Failed to set device metrics")?;

    // Navigate
    let nav_start = Instant::now();
    tab.navigate_to(url)
        .context("Failed to navigate to URL")?;
    tab.wait_until_navigated()
        .context("Timed out waiting for navigation")?;
    let navigate_ms = nav_start.elapsed().as_millis();

    // Inject animation-disabling CSS
    let css_js = format!(
        r#"(() => {{
            const style = document.createElement('style');
            style.textContent = {};
            document.head.appendChild(style);
        }})()"#,
        serde_json_inline(DISABLE_ANIMATIONS_CSS),
    );
    tab.evaluate(&css_js, false)
        .context("Failed to inject animation-disabling CSS")?;

    // Wait for ready
    let ready_start = Instant::now();
    tab.evaluate(WAIT_FOR_READY_JS, true)
        .context("Ready detection failed")?;
    let ready_ms = ready_start.elapsed().as_millis();

    // Get body bounds
    let bounds_result = tab
        .evaluate(GET_BODY_BOUNDS_JS, false)
        .context("Failed to get body bounds")?;
    let bounds_json = bounds_result
        .value
        .as_ref()
        .and_then(|v| v.as_str())
        .context("Body bounds returned no value")?;
    let bounds: BodyBounds =
        serde_json_parse(bounds_json).context("Failed to parse body bounds")?;

    // Screenshot clipped to body
    let screenshot_start = Instant::now();
    let viewport = Page::Viewport {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
        scale: scale as f64,
    };
    let png = tab
        .capture_screenshot(
            Page::CaptureScreenshotFormatOption::Png,
            None,
            Some(viewport),
            true,
        )
        .context("Failed to capture screenshot")?;
    let screenshot_ms = screenshot_start.elapsed().as_millis();

    Ok(CaptureResult {
        png,
        body_x: bounds.x,
        body_y: bounds.y,
        body_width: bounds.width,
        body_height: bounds.height,
        navigate_ms,
        ready_ms,
        screenshot_ms,
    })
}

/// Single URL mode.
fn run_single(url: &str, output: &PathBuf, width: u32, height: u32, scale: u32) -> Result<()> {
    let total_start = Instant::now();

    let launch_start = Instant::now();
    let options = LaunchOptions {
        headless: true,
        window_size: Some((width, height)),
        sandbox: true,
        ..LaunchOptions::default()
    };
    let browser = Browser::new(options).context("Failed to launch Chrome")?;
    let launch_ms = launch_start.elapsed().as_millis();

    let tab = browser.new_tab().context("Failed to create tab")?;
    tab.set_default_timeout(std::time::Duration::from_secs(30));

    let result = capture_story(&tab, url, width, height, scale)?;

    fs::write(output, &result.png)
        .with_context(|| format!("Failed to write {}", output.display()))?;

    let total_ms = total_start.elapsed().as_millis();

    println!("Screenshot saved to: {}", output.display());
    println!("Image size: {} bytes", result.png.len());
    print_body_bounds(&result);
    println!();
    println!("Timing:");
    println!("  Launch:     {:>6}ms", launch_ms);
    print_capture_timing(&result);
    println!("  Total:      {:>6}ms", total_ms);

    Ok(())
}

/// Parallel mode: N tabs in ONE browser, each on its own OS thread.
///
/// headless_chrome is synchronous and uses Arc<Tab> which is Send + Sync,
/// so real OS threads give us true parallelism. This tests whether
/// headless_chrome's transport handles concurrent tab operations better
/// than chromiumoxide's single-handler bottleneck.
fn run_parallel(concurrency: usize, width: u32, height: u32, scale: u32) -> Result<()> {
    let total_start = Instant::now();

    let launch_start = Instant::now();
    let options = LaunchOptions {
        headless: true,
        window_size: Some((width, height)),
        sandbox: true,
        ..LaunchOptions::default()
    };
    let browser = Browser::new(options).context("Failed to launch Chrome")?;
    let launch_ms = launch_start.elapsed().as_millis();

    // Create N tabs up front.
    let mut tabs = Vec::with_capacity(concurrency);
    for _ in 0..concurrency {
        let tab = browser.new_tab().context("Failed to create tab")?;
        tab.set_default_timeout(std::time::Duration::from_secs(30));
        tabs.push(tab);
    }

    // Split stories round-robin across tabs.
    let mut tab_queues: Vec<Vec<&str>> = vec![vec![]; concurrency];
    for (i, story) in EXAMPLE_STORIES.iter().enumerate() {
        tab_queues[i % concurrency].push(story);
    }

    let out_dir = PathBuf::from("screenshots");
    fs::create_dir_all(&out_dir).context("Failed to create screenshots dir")?;

    println!(
        "Capturing {} stories with {} tab(s) in 1 browser...",
        EXAMPLE_STORIES.len(),
        concurrency
    );
    println!();

    let capture_start = Instant::now();

    // Use std::thread::scope so threads can borrow tabs and queues.
    let all_results: Vec<Vec<(String, Result<CaptureResult>)>> =
        std::thread::scope(|s| {
            let mut handles = Vec::with_capacity(concurrency);
            for (tab_idx, stories) in tab_queues.into_iter().enumerate() {
                let tab = &tabs[tab_idx];
                handles.push(s.spawn(move || {
                    let mut results = Vec::new();
                    for story in stories {
                        let url = format!("{STORYBOOK_BASE}{story}");
                        let result = capture_story(tab, &url, width, height, scale);
                        results.push((story.to_string(), result));
                    }
                    results
                }));
            }
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

    let capture_wall_ms = capture_start.elapsed().as_millis();

    let results: Vec<(String, Result<CaptureResult>)> =
        all_results.into_iter().flatten().collect();

    // Write results and print timing.
    for (story, res) in &results {
        match res {
            Ok(result) => {
                let path = out_dir.join(format!("{story}.png"));
                fs::write(&path, &result.png)
                    .with_context(|| format!("Failed to write {}", path.display()))?;
                println!("{story}:");
                println!(
                    "  {} bytes, body {:.0}x{:.0}",
                    result.png.len(),
                    result.body_width,
                    result.body_height
                );
                print_capture_timing(result);
            }
            Err(e) => {
                println!("{story}: ERROR: {e:#}");
            }
        }
    }

    let total_ms = total_start.elapsed().as_millis();
    let succeeded = results.iter().filter(|(_, r)| r.is_ok()).count();
    let failed = results.iter().filter(|(_, r)| r.is_err()).count();

    println!();
    println!("Summary:");
    println!("  Stories:    {succeeded} ok, {failed} failed");
    println!("  Tabs:       {concurrency}");
    println!("  Launch:     {:>6}ms", launch_ms);
    println!("  Captures:   {:>6}ms (wall)", capture_wall_ms);
    println!("  Total:      {:>6}ms", total_ms);

    Ok(())
}

struct CaptureResult {
    png: Vec<u8>,
    body_x: f64,
    body_y: f64,
    body_width: f64,
    body_height: f64,
    navigate_ms: u128,
    ready_ms: u128,
    screenshot_ms: u128,
}

/// Body bounding rectangle from getBoundingClientRect().
struct BodyBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn print_body_bounds(result: &CaptureResult) {
    println!(
        "  Body bounds: {:.0}x{:.0} at ({:.0},{:.0})",
        result.body_width, result.body_height, result.body_x, result.body_y,
    );
}

fn print_capture_timing(result: &CaptureResult) {
    println!("  Navigate:   {:>6}ms", result.navigate_ms);
    println!("  Ready:      {:>6}ms", result.ready_ms);
    println!("  Screenshot: {:>6}ms", result.screenshot_ms);
}

/// Minimal JSON string escaping for embedding CSS in JS.
fn serde_json_inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str(r#"\""#),
            '\\' => out.push_str(r"\\"),
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            '\t' => out.push_str(r"\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Minimal JSON parsing for the body bounds object.
fn serde_json_parse(json: &str) -> Result<BodyBounds> {
    // Parse a simple {"x":0,"y":0,"width":100,"height":200} object
    let json = json.trim();
    let json = json
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .context("Expected JSON object")?;

    let mut x = None;
    let mut y = None;
    let mut width = None;
    let mut height = None;

    for pair in json.split(',') {
        let pair = pair.trim();
        let (key, val) = pair.split_once(':').context("Expected key:value pair")?;
        let key = key.trim().trim_matches('"');
        let val: f64 = val.trim().parse().context("Expected numeric value")?;
        match key {
            "x" => x = Some(val),
            "y" => y = Some(val),
            "width" => width = Some(val),
            "height" => height = Some(val),
            _ => {}
        }
    }

    Ok(BodyBounds {
        x: x.context("Missing x")?,
        y: y.context("Missing y")?,
        width: width.context("Missing width")?,
        height: height.context("Missing height")?,
    })
}
