use std::collections::BTreeSet;
use std::time::Instant;

use anyhow::{Context, Result};
use tracing::debug;

use crate::capture::{CaptureOutcome, CapturePlan, CaptureTimings};
use crate::compare::SnapshotStatus;
use crate::compare::diff;
use crate::config::ResolvedRunConfig;
use crate::report::terminal;
use crate::store;

/// `snapvrt test` — discover, capture, compare, report.
/// Returns exit code: 0 = all pass, 1 = any fail or new.
pub async fn test(
    config: ResolvedRunConfig,
    filter: Option<&str>,
    timings: bool,
    prune: bool,
) -> Result<i32> {
    let threshold = config.diff_threshold;
    let run = CapturePlan::plan(&config, filter).await?;
    if run.total() == 0 {
        return Ok(0);
    }

    // Save planned IDs before execute() consumes the plan.
    let planned_ids: BTreeSet<String> = run.job_names().into_iter().collect();

    // Clear stale current/difference files before capturing.
    // Full run: wipe both dirs (catches removed/renamed stories).
    // Filtered run: only clear files for the snapshots being tested.
    if filter.is_some() {
        store::clean_output_files(&run.job_names());
    } else {
        store::clear_output_dirs();
    }

    let run_start = Instant::now();
    let total = run.total();
    let mut rx = run.execute().await?;

    let mut done = 0usize;
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut new = 0usize;
    let mut errored = 0usize;
    let mut all_timings: Vec<(String, CaptureTimings)> = Vec::new();

    let mut failed_names: Vec<String> = Vec::new();
    let mut new_names: Vec<String> = Vec::new();
    let mut errored_names: Vec<String> = Vec::new();

    debug!(total, "waiting for capture results");
    while let Some((job, outcome)) = rx.recv().await {
        done += 1;
        let name = job.snapshot_id();
        debug!(done, total, name = %name, "received result");
        let (current_png, mut timings) = match outcome {
            CaptureOutcome::Ok(png, timings) => (png, timings),
            CaptureOutcome::Err(msg) => {
                errored += 1;
                errored_names.push(name.clone());
                terminal::print_error_line(&name, &msg);
                terminal::show_progress(done, total);
                continue;
            }
        };

        let reference_png = store::read_reference(&name);

        let status = match reference_png {
            Some(ref_png) => {
                let cur_png = current_png.clone();
                let t_compare = Instant::now();
                let compare_result =
                    tokio::task::spawn_blocking(move || diff::compare(&ref_png, &cur_png))
                        .await
                        .context("Diff task panicked")
                        .and_then(|r| r);
                timings.compare = t_compare.elapsed();

                match compare_result {
                    Err(e) => {
                        store::write_current(&name, &current_png)?;
                        SnapshotStatus::Error(format!("{e:#}"))
                    }
                    Ok(result) if result.is_match || result.score <= threshold => {
                        store::clean_output(&name);
                        SnapshotStatus::Pass
                    }
                    Ok(result) => {
                        store::write_current(&name, &current_png)?;
                        if let Some(diff_img) = &result.diff_image {
                            let mut diff_png = Vec::new();
                            diff_img
                                .write_to(
                                    &mut std::io::Cursor::new(&mut diff_png),
                                    image::ImageFormat::Png,
                                )
                                .context("Failed to encode diff image")?;
                            store::write_difference(&name, &diff_png)?;
                        }
                        SnapshotStatus::Fail {
                            diff_pixels: result.diff_pixels,
                            score: result.score,
                            dimension_mismatch: result.dimension_mismatch,
                        }
                    }
                }
            }
            None => {
                store::write_current(&name, &current_png)?;
                SnapshotStatus::New
            }
        };

        match &status {
            SnapshotStatus::Pass => passed += 1,
            SnapshotStatus::Fail { .. } => {
                failed += 1;
                failed_names.push(name.clone());
            }
            SnapshotStatus::New => {
                new += 1;
                new_names.push(name.clone());
            }
            SnapshotStatus::Error(_) => {
                errored += 1;
                errored_names.push(name.clone());
            }
        }

        terminal::print_line(&name, &status, timings.total + timings.compare);
        all_timings.push((name, timings));
        terminal::show_progress(done, total);
    }

    // Orphan detection: only on full (unfiltered) runs.
    let mut removed_names: Vec<String> = Vec::new();
    if filter.is_none() {
        let reference_ids = store::list_reference_ids();
        let orphans: BTreeSet<&String> = reference_ids.difference(&planned_ids).collect();
        for id in &orphans {
            terminal::print_removed_line(id);
            removed_names.push((*id).clone());
        }
        if prune {
            for id in &orphans {
                store::remove_reference(id);
            }
        }
    }

    if timings {
        terminal::print_timing_table(&all_timings);
        terminal::print_timing_summary(&all_timings);
    }

    terminal::print_actionable_summary(&failed_names, &new_names, &errored_names, &removed_names);
    terminal::print_summary(
        total,
        passed,
        failed,
        new,
        errored,
        removed_names.len(),
        run_start.elapsed(),
    );

    // Removed snapshots do NOT affect exit code.
    if failed > 0 || new > 0 || errored > 0 {
        Ok(1)
    } else {
        Ok(0)
    }
}
