use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::sync::{Mutex, mpsc};
use tracing::{Instrument, debug, debug_span, info_span, warn};

use super::job::CaptureJob;
use super::pipeline::{CaptureRequest, CdpRenderer};
use super::timing::CaptureTimings;
use crate::config::CaptureConfig;

/// Per-capture timeout. Covers navigate + load + network idle + ready + screenshot.
/// Must exceed the sum of individual stage timeouts (network: 10s, ready JS: 10s,
/// stable screenshot: ~600ms) plus time for Chrome to actually load the page.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(30);

/// Per-snapshot capture outcome.
pub enum CaptureOutcome {
    Ok(Vec<u8>, CaptureTimings),
    Err(String),
}

/// Drain remaining jobs from the queue, reporting each as a Chrome crash error.
async fn drain_crashed(
    queue: &Mutex<Vec<CaptureJob>>,
    tx: &mpsc::Sender<(CaptureJob, CaptureOutcome)>,
) {
    while let Some(job) = queue.lock().await.pop() {
        let _ = tx
            .send((job, CaptureOutcome::Err("Chrome process crashed".into())))
            .await;
    }
}

/// Capture a pre-built list of jobs.
///
/// Individual capture failures are reported per-snapshot rather than aborting the run.
///
/// Returns a `Receiver` — results stream in as captures complete.
pub async fn capture_all(
    jobs: Vec<CaptureJob>,
    config: &CaptureConfig,
) -> Result<mpsc::Receiver<(CaptureJob, CaptureOutcome)>> {
    if jobs.is_empty() {
        let (_tx, rx) = mpsc::channel(1);
        return Ok(rx);
    }

    let parallel = config.parallel();
    let renderer = CdpRenderer::launch(config).await?;
    capture_all_with(renderer, jobs, parallel).await
}

/// Capture orchestration: creates parallel workers with a shared work queue.
///
/// Each capture gets a fresh tab to avoid browser-level WS mutex contention.
///
/// Returns a `Receiver` immediately — captures stream in via the channel.
async fn capture_all_with(
    renderer: CdpRenderer,
    jobs: Vec<CaptureJob>,
    parallel: usize,
) -> Result<mpsc::Receiver<(CaptureJob, CaptureOutcome)>> {
    let job_count = jobs.len();
    let worker_count = job_count.min(parallel.max(1));
    debug!(
        jobs = job_count,
        workers = worker_count,
        parallel,
        "starting capture run"
    );

    /// Consecutive session-creation failures before we declare Chrome dead.
    const MAX_SESSION_FAILURES: u32 = 3;

    let renderer = Arc::new(renderer);
    let queue = Arc::new(Mutex::new(jobs));
    let chrome_dead = Arc::new(AtomicBool::new(false));

    let (tx, rx) = mpsc::channel(parallel.max(1) * 2);

    // Spawn one task per worker — each pulls from the shared queue
    let mut set = tokio::task::JoinSet::new();
    for idx in 0..worker_count {
        let queue = queue.clone();
        let tx = tx.clone();
        let renderer = renderer.clone();
        let chrome_dead = chrome_dead.clone();
        let span = info_span!("worker", id = idx);
        set.spawn(
            async move {
                debug!("started");
                let mut consecutive_session_failures: u32 = 0;

                loop {
                    // If another worker detected Chrome is dead, drain and exit.
                    if chrome_dead.load(Ordering::Relaxed) {
                        debug!("chrome is dead, draining remaining jobs");
                        drain_crashed(&queue, &tx).await;
                        break;
                    }

                    let (job, remaining) = {
                        let mut q = queue.lock().await;
                        match q.pop() {
                            Some(j) => {
                                let remaining = q.len();
                                (j, remaining)
                            }
                            None => {
                                debug!("queue empty, exiting");
                                break;
                            }
                        }
                    };
                    debug!(job = %job.snapshot_id(), remaining, "picked job");

                    // Create a fresh session (tab) for each capture.
                    let t_create = Instant::now();
                    let mut session = match renderer.new_session().await {
                        Ok(s) => {
                            consecutive_session_failures = 0;
                            debug!(
                                target_id = %s.target_id(),
                                elapsed_ms = t_create.elapsed().as_millis() as u64,
                                "session created"
                            );
                            s
                        }
                        Err(e) => {
                            consecutive_session_failures += 1;
                            warn!(
                                error = %format!("{e:#}"),
                                consecutive = consecutive_session_failures,
                                "failed to create session"
                            );
                            let _ = tx
                                .send((
                                    job,
                                    CaptureOutcome::Err(format!("Session creation failed: {e:#}")),
                                ))
                                .await;

                            if consecutive_session_failures >= MAX_SESSION_FAILURES {
                                warn!(
                                    "Chrome appears to have crashed \
                                     ({consecutive_session_failures} consecutive session failures), \
                                     aborting remaining captures"
                                );
                                chrome_dead.store(true, Ordering::Relaxed);
                                drain_crashed(&queue, &tx).await;
                                break;
                            }
                            continue;
                        }
                    };

                    let req = CaptureRequest {
                        url: job.url.clone(),
                        width: job.width,
                        height: job.height,
                    };
                    let capture_span = debug_span!("capture", job = %job.snapshot_id());
                    let outcome = match tokio::time::timeout(
                        CAPTURE_TIMEOUT,
                        session.capture(&req).instrument(capture_span),
                    )
                    .await
                    {
                        Ok(Ok(result)) => {
                            debug!(
                                elapsed_ms = result.timings.total.as_millis() as u64,
                                "captured ok"
                            );
                            CaptureOutcome::Ok(result.png, result.timings)
                        }
                        Ok(Err(e)) => {
                            warn!(error = %format!("{e:#}"), "capture failed");
                            CaptureOutcome::Err(format!("{e:#}"))
                        }
                        Err(_) => {
                            warn!("capture timed out after 30s");
                            // Timeout — close the tab and continue with next job.
                            let _ = renderer.close_session(session).await;
                            let _ = tx
                                .send((
                                    job,
                                    CaptureOutcome::Err("Capture timed out after 30s".into()),
                                ))
                                .await;
                            continue;
                        }
                    };

                    // Close the tab after capture.
                    let t_close = Instant::now();
                    if let Err(e) = renderer.close_session(session).await {
                        warn!(error = %format!("{e:#}"), "failed to close tab");
                    } else {
                        debug!(
                            elapsed_ms = t_close.elapsed().as_millis() as u64,
                            "tab closed"
                        );
                    }

                    if tx.send((job, outcome)).await.is_err() {
                        warn!("channel send failed (receiver dropped), stopping");
                        break; // receiver dropped, stop capturing
                    }
                }
                debug!("exiting");
            }
            .instrument(span),
        );
    }

    // Original sender not needed — channel closes when session task clones drop.
    drop(tx);
    debug!("original tx dropped, channel will close when all workers finish");

    // Keep Chrome alive until all captures finish.
    tokio::spawn(async move {
        let _renderer = renderer;
        debug!("renderer keep-alive task started");
        while let Some(result) = set.join_next().await {
            match result {
                Ok(()) => debug!("worker task joined"),
                Err(e) => warn!(error = %e, "worker task panicked"),
            }
        }
        debug!("all workers done, dropping renderer");
    });

    Ok(rx)
}
