use std::time::Instant;

use anyhow::Result;
use tracing::debug;

use crate::capture::{CaptureOutcome, CapturePlan, CaptureTimings};
use crate::config::ResolvedRunConfig;
use crate::report::terminal;
use crate::store;

/// `snapvrt update` — discover, capture, save as references.
pub async fn update(config: ResolvedRunConfig, filter: Option<&str>, timings: bool) -> Result<()> {
    let run = CapturePlan::plan(&config, filter).await?;
    if run.total() == 0 {
        return Ok(());
    }

    let run_start = Instant::now();
    let total = run.total();
    let mut rx = run.execute().await?;

    let mut done = 0usize;
    let mut saved = 0usize;
    let mut errored = 0usize;
    let mut all_timings: Vec<(String, CaptureTimings)> = Vec::new();
    debug!(total, "waiting for capture results");
    while let Some((job, outcome)) = rx.recv().await {
        done += 1;
        let name = job.snapshot_id();
        debug!(done, total, name = %name, "received result");
        match outcome {
            CaptureOutcome::Ok(png, timings) => {
                terminal::clear_line();
                store::write_reference(&name, &png)?;
                println!(
                    "  Updated  {name}  \x1b[2m{}ms\x1b[0m",
                    timings.total.as_millis()
                );
                all_timings.push((name, timings));
                saved += 1;
            }
            CaptureOutcome::Err(msg) => {
                terminal::print_error_line(&name, &msg);
                errored += 1;
            }
        }
        terminal::show_progress(done, total);
    }

    if timings {
        terminal::print_timing_table(&all_timings);
        terminal::print_timing_summary(&all_timings);
    }

    println!();
    println!("{saved} reference snapshot(s) saved.");
    if errored > 0 {
        println!("{errored} snapshot(s) failed to capture.");
    }
    println!("Time: {}", terminal::format_duration(run_start.elapsed()));

    Ok(())
}
