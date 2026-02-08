use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use poc_cdp_raw::capture::{CaptureRequest, CaptureResult, capture};
use poc_cdp_raw::cdp::CdpConnection;
use poc_cdp_raw::chrome::Chrome;

/// PoC: CDP screenshot capture via raw per-target WebSockets.
///
/// Three modes:
///   --url <URL>         Single URL screenshot
///   --parallel <N>      N tabs in 1 browser, each with own WebSocket
///   --test-viewports    3 viewports on 3 tabs in parallel (same story)
#[derive(Parser)]
#[command(name = "poc-cdp-raw")]
struct Cli {
    /// Single URL mode: URL to screenshot
    #[arg(long)]
    url: Option<String>,

    /// Output file path (single mode only)
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

    /// Multi-viewport mode: 3 viewports on 3 tabs in parallel
    #[arg(long)]
    test_viewports: bool,
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

struct ViewportConfig {
    name: &'static str,
    width: u32,
    height: u32,
    scale: u32,
}

const TEST_VIEWPORTS: &[ViewportConfig] = &[
    ViewportConfig {
        name: "desktop",
        width: 1366,
        height: 768,
        scale: 1,
    },
    ViewportConfig {
        name: "mobile",
        width: 375,
        height: 667,
        scale: 2,
    },
    ViewportConfig {
        name: "tablet",
        width: 768,
        height: 1024,
        scale: 1,
    },
];

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.test_viewports {
        run_test_viewports().await
    } else if let Some(n) = cli.parallel {
        run_parallel(n, cli.width, cli.height, cli.scale).await
    } else if let Some(url) = &cli.url {
        run_single(url, &cli.output, cli.width, cli.height, cli.scale).await
    } else {
        anyhow::bail!("Specify one of: --url <URL>, --parallel <N>, or --test-viewports");
    }
}

/// Single URL mode: one URL, one screenshot, print timing.
async fn run_single(
    url: &str,
    output: &PathBuf,
    width: u32,
    height: u32,
    scale: u32,
) -> Result<()> {
    let total_start = Instant::now();

    let launch_start = Instant::now();
    let mut chrome = Chrome::launch().await?;
    let launch_ms = launch_start.elapsed().as_millis();

    let (_target_id, ws_url) = chrome.create_tab().await?;
    let mut conn = CdpConnection::connect(&ws_url).await?;

    let result = capture(
        &mut conn,
        &CaptureRequest {
            url: url.to_string(),
            width,
            height,
            scale,
        },
    )
    .await?;

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

    conn.close().await.ok();
    chrome.kill();
    Ok(())
}

/// Parallel mode: N tabs in 1 browser, each with its own WebSocket connection.
///
/// This is the key test: per-target WebSocket gives each tab a dedicated
/// connection with no shared transport, no contention, true parallelism.
/// Compare with chromiumoxide (broken multi-tab) and headless_chrome (4x slower).
async fn run_parallel(concurrency: usize, width: u32, height: u32, scale: u32) -> Result<()> {
    let total_start = Instant::now();

    // Split stories into per-worker queues (round-robin).
    let mut worker_queues: Vec<Vec<String>> = vec![vec![]; concurrency];
    for (i, story) in EXAMPLE_STORIES.iter().enumerate() {
        worker_queues[i % concurrency].push(story.to_string());
    }

    let out_dir = PathBuf::from("screenshots");
    fs::create_dir_all(&out_dir).context("Failed to create screenshots dir")?;

    println!(
        "Capturing {} stories with {} tab(s) in 1 browser...",
        EXAMPLE_STORIES.len(),
        concurrency
    );
    println!();

    let launch_start = Instant::now();
    let chrome = Chrome::launch().await?;
    let launch_ms = launch_start.elapsed().as_millis();
    println!("  Browser launched in {launch_ms}ms");

    // Create N tabs, each with its own per-target WebSocket.
    let tab_start = Instant::now();
    let mut tab_urls = Vec::with_capacity(concurrency);
    for _ in 0..concurrency {
        let (_target_id, ws_url) = chrome.create_tab().await?;
        tab_urls.push(ws_url);
    }
    let tab_ms = tab_start.elapsed().as_millis();
    println!("  Created {concurrency} tab(s) in {tab_ms}ms");

    let capture_start = Instant::now();

    // Spawn N workers, each connecting to its own tab's WebSocket.
    // No shared transport, no contention — true parallelism.
    let mut handles = Vec::with_capacity(concurrency);
    for (ws_url, stories) in tab_urls.into_iter().zip(worker_queues) {
        handles.push(tokio::spawn(async move {
            let mut conn = CdpConnection::connect(&ws_url).await?;

            let mut results = Vec::new();
            for story in stories {
                let url = format!("{STORYBOOK_BASE}{story}");
                let result = capture(
                    &mut conn,
                    &CaptureRequest {
                        url,
                        width,
                        height,
                        scale,
                    },
                )
                .await;
                results.push((story, result));
            }

            conn.close().await.ok();
            Ok::<_, anyhow::Error>(results)
        }));
    }

    // Collect results from all workers.
    let mut results: Vec<(String, Result<CaptureResult>)> = Vec::new();
    for handle in handles {
        match handle.await.context("Worker task panicked")? {
            Ok(worker_results) => results.extend(worker_results),
            Err(e) => println!("Worker failed: {e:#}"),
        }
    }

    let capture_wall_ms = capture_start.elapsed().as_millis();

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
    println!("  Tabs:       {concurrency} (1 browser)");
    println!("  Launch:     {:>6}ms", launch_ms);
    println!("  Tab create: {:>6}ms", tab_ms);
    println!("  Captures:   {:>6}ms (wall)", capture_wall_ms);
    println!("  Total:      {:>6}ms", total_ms);

    drop(chrome);
    Ok(())
}

/// Multi-viewport mode: 3 tabs in 1 browser with different viewports in parallel, same story.
async fn run_test_viewports() -> Result<()> {
    let total_start = Instant::now();

    let story = "example-button--primary";

    let out_dir = PathBuf::from("screenshots");
    fs::create_dir_all(&out_dir).context("Failed to create screenshots dir")?;

    println!("Capturing {story} in {} viewports...", TEST_VIEWPORTS.len());
    println!();

    let chrome = Chrome::launch().await?;

    let capture_start = Instant::now();

    // One tab per viewport, all in the same browser.
    let mut handles = Vec::with_capacity(TEST_VIEWPORTS.len());
    for (i, vp) in TEST_VIEWPORTS.iter().enumerate() {
        let (_target_id, ws_url) = chrome.create_tab().await?;
        let url = format!("{STORYBOOK_BASE}{story}");
        let width = vp.width;
        let height = vp.height;
        let scale = vp.scale;
        handles.push(tokio::spawn(async move {
            let mut conn = CdpConnection::connect(&ws_url).await?;
            let result = capture(
                &mut conn,
                &CaptureRequest {
                    url,
                    width,
                    height,
                    scale,
                },
            )
            .await;
            conn.close().await.ok();
            Ok::<_, anyhow::Error>((i, result))
        }));
    }

    let mut results: Vec<(usize, Result<CaptureResult>)> = Vec::new();
    for handle in handles {
        match handle.await.context("Viewport task panicked")? {
            Ok((i, result)) => results.push((i, result)),
            Err(e) => println!("Viewport worker failed: {e:#}"),
        }
    }

    let capture_wall_ms = capture_start.elapsed().as_millis();

    for (i, res) in &results {
        let vp = &TEST_VIEWPORTS[*i];
        match res {
            Ok(result) => {
                let path = out_dir.join(format!("{}.png", vp.name));
                fs::write(&path, &result.png)
                    .with_context(|| format!("Failed to write {}", path.display()))?;
                println!("{}  ({}x{} @{}x):", vp.name, vp.width, vp.height, vp.scale);
                println!("  {} bytes", result.png.len());
                print_body_bounds(result);
                print_capture_timing(result);
            }
            Err(e) => {
                println!(
                    "{}  ({}x{} @{}x): ERROR: {e:#}",
                    vp.name, vp.width, vp.height, vp.scale
                );
            }
        }
    }

    let total_ms = total_start.elapsed().as_millis();

    println!();
    println!("Summary:");
    println!("  Captures:   {:>6}ms (wall)", capture_wall_ms);
    println!("  Total:      {:>6}ms", total_ms);

    drop(chrome);
    Ok(())
}

fn print_body_bounds(result: &CaptureResult) {
    println!(
        "  Body bounds: {:.0}x{:.0} at ({:.0},{:.0})",
        result.body_width, result.body_height, result.body_x, result.body_y,
    );
}

fn print_capture_timing(result: &CaptureResult) {
    let t = &result.timing;
    println!("  Navigate:   {:>6}ms", t.navigate_ms);
    println!("  Ready:      {:>6}ms", t.ready_ms);
    println!("  Screenshot: {:>6}ms", t.screenshot_ms);
}
