use std::io::Write;
use std::time::Duration;

use crate::capture::CaptureTimings;
use crate::compare::SnapshotStatus;

const STAGE_NAMES: [&str; 10] = [
    "viewport",
    "navigate",
    "page_load",
    "network",
    "animation",
    "ready",
    "selector",
    "clip",
    "screenshot",
    "compare",
];

fn stage_durations(t: &CaptureTimings) -> [Duration; 10] {
    [
        t.viewport,
        t.navigate,
        t.page_load,
        t.network,
        t.animation,
        t.ready,
        t.selector,
        t.clip,
        t.screenshot,
        t.compare,
    ]
}

/// Clear the current terminal line (wipes progress indicator).
pub fn clear_line() {
    print!("\r\x1b[2K");
}

pub fn format_duration(d: Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", d.as_secs_f64())
    }
}

/// Print a single snapshot result line.
pub fn print_line(name: &str, status: &SnapshotStatus, elapsed: Duration) {
    clear_line();
    let time_suffix = format!("  \x1b[2m{}\x1b[0m", format_duration(elapsed));

    match status {
        SnapshotStatus::Pass => {
            println!("  \x1b[32mPASS\x1b[0m  {name}{time_suffix}");
        }
        SnapshotStatus::Fail {
            diff_pixels,
            score,
            dimension_mismatch,
        } => {
            if let Some((rw, rh, cw, ch)) = dimension_mismatch {
                println!(
                    "  \x1b[31mFAIL\x1b[0m  {name}  (dimensions changed: {rw}x{rh} -> {cw}x{ch}){time_suffix}"
                );
            } else {
                println!(
                    "  \x1b[31mFAIL\x1b[0m  {name}  ({diff_pixels} pixels, {score:.4}){time_suffix}"
                );
            }
        }
        SnapshotStatus::New => {
            println!("  \x1b[33m NEW\x1b[0m  {name}  (no reference){time_suffix}");
        }
        SnapshotStatus::Error(msg) => {
            println!("  \x1b[31m ERR\x1b[0m  {name}  ({msg}){time_suffix}");
        }
    }
}

/// Print an error line (no timing available).
pub fn print_error_line(name: &str, msg: &str) {
    clear_line();
    println!("  \x1b[31m ERR\x1b[0m  {name}  ({msg})");
}

/// Print a removed/orphaned reference line.
pub fn print_removed_line(name: &str) {
    clear_line();
    println!("  \x1b[2mGONE\x1b[0m  \x1b[2m{name}  (no matching story)\x1b[0m");
}

/// Show capture progress indicator.
pub fn show_progress(done: usize, total: usize) {
    if done < total {
        print!("  Capturing  [{done}/{total}]");
        let _ = std::io::stdout().flush();
    }
}

/// Print an actionable summary listing snapshot names grouped by status.
/// Only prints sections with at least one entry.
pub fn print_actionable_summary(
    failed: &[String],
    new: &[String],
    errored: &[String],
    removed: &[String],
) {
    if failed.is_empty() && new.is_empty() && errored.is_empty() && removed.is_empty() {
        return;
    }

    clear_line();
    println!();
    println!("Actionable snapshots:");

    for (label, names) in [
        ("Failed", failed),
        ("New", new),
        ("Errored", errored),
        ("Removed", removed),
    ] {
        if !names.is_empty() {
            println!();
            println!("  {label} ({}):", names.len());
            for name in names {
                println!("    {name}");
            }
        }
    }
}

/// Print the final summary.
pub fn print_summary(
    total: usize,
    passed: usize,
    failed: usize,
    new: usize,
    errored: usize,
    removed: usize,
    elapsed: Duration,
) {
    clear_line();
    println!();
    print!(
        "Snapshots:  {total} total, \x1b[32m{passed} passed\x1b[0m, \x1b[31m{failed} failed\x1b[0m, \x1b[33m{new} new\x1b[0m"
    );
    if errored > 0 {
        print!(", \x1b[31m{errored} errored\x1b[0m");
    }
    if removed > 0 {
        print!(", \x1b[2m{removed} removed\x1b[0m");
    }
    println!();
    println!("Time:       {}", format_duration(elapsed));

    if failed > 0 || new > 0 || errored > 0 || removed > 0 {
        println!();
        if failed > 0 {
            println!("{failed} snapshot(s) have visual differences.");
        }
        if new > 0 {
            println!("{new} snapshot(s) have no reference.");
        }
        if errored > 0 {
            println!("{errored} snapshot(s) failed to capture.");
        }
        if removed > 0 {
            println!(
                "{removed} reference(s) no longer match any story. Run `snapvrt prune` to delete."
            );
        }
        println!("Run `snapvrt approve` to accept, or `snapvrt update` to re-capture.");
    }
}

/// Print a per-snapshot timing table with all stage breakdowns.
///
/// Sorted by total descending (slowest first). Right-aligned numeric columns.
pub fn print_timing_table(entries: &[(String, CaptureTimings)]) {
    if entries.is_empty() {
        return;
    }

    let mut sorted: Vec<&(String, CaptureTimings)> = entries.iter().collect();
    sorted.sort_by(|a, b| {
        let a_total = a.1.total + a.1.compare;
        let b_total = b.1.total + b.1.compare;
        b_total.cmp(&a_total)
    });

    // Find the longest snapshot name for column width (min 8, max 50).
    let name_width = sorted
        .iter()
        .map(|(n, _)| n.len())
        .max()
        .unwrap_or(8)
        .clamp(8, 50);

    let headers = [
        "total", "viewpt", "navig", "load", "network", "anim", "ready", "select", "clip", "screen",
        "compare",
    ];

    println!();
    println!("\x1b[1mCapture timings (all snapshots):\x1b[0m");
    println!();

    // Header line.
    print!("  {:<width$}", "Snapshot", width = name_width);
    for h in &headers {
        print!("  {:>7}", h);
    }
    println!();

    // Separator line.
    let sep_len = name_width + 2 + headers.len() * 9;
    print!("  ");
    for _ in 0..sep_len {
        print!("\u{2500}");
    }
    println!();

    // Data rows.
    for (name, t) in &sorted {
        let display_name = truncate_name(name, name_width);
        print!("  {:<width$}", display_name, width = name_width);
        print!("  {:>5}ms", (t.total + t.compare).as_millis());
        for d in stage_durations(t) {
            print!("  {:>5}ms", d.as_millis());
        }
        println!();
    }
}

/// Print an aggregate timing breakdown across all captured snapshots.
///
/// Shows average time per stage (sorted by descending avg), a proportional bar
/// chart, and the top 5 slowest snapshots with their dominant stage.
///
/// Only prints when there are 2+ entries (a single snapshot isn't interesting).
pub fn print_timing_summary(entries: &[(String, CaptureTimings)]) {
    if entries.len() < 2 {
        return;
    }

    let n = entries.len() as u128;

    // Accumulate totals per stage.
    let mut stage_sums = [0u128; 10];
    for (_, t) in entries {
        for (i, d) in stage_durations(t).iter().enumerate() {
            stage_sums[i] += d.as_millis();
        }
    }

    let stage_avgs: Vec<u128> = stage_sums.iter().map(|s| s / n).collect();
    let total_avg: u128 = stage_avgs.iter().sum();

    // Sort stages by avg descending.
    let mut indexed: Vec<(usize, u128)> = stage_avgs.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.cmp(&a.1));

    // Max bar width in characters.
    const BAR_MAX: usize = 20;
    let max_avg = indexed.first().map_or(1, |&(_, v)| v.max(1));

    println!();
    println!("\x1b[1mTiming breakdown\x1b[0m (avg per snapshot):");

    for &(i, avg) in &indexed {
        let pct = if total_avg > 0 {
            (avg as f64 / total_avg as f64) * 100.0
        } else {
            0.0
        };
        let bar_len = ((avg as f64 / max_avg as f64) * BAR_MAX as f64).round() as usize;
        let bar: String = "\u{2588}".repeat(bar_len);
        let pct_str = if pct < 1.0 {
            "<1%".to_string()
        } else {
            format!("{:.0}%", pct)
        };
        println!(
            "  {:<12} {:>4}ms  {:<width$}  {:>4}",
            STAGE_NAMES[i],
            avg,
            bar,
            pct_str,
            width = BAR_MAX,
        );
    }

    // Top 5 slowest snapshots.
    let mut by_total: Vec<(usize, u128)> = entries
        .iter()
        .enumerate()
        .map(|(i, (_, t))| (i, (t.total + t.compare).as_millis()))
        .collect();
    by_total.sort_by(|a, b| b.1.cmp(&a.1));

    let top_n = by_total.len().min(5);
    println!();
    println!("\x1b[1mSlowest snapshots:\x1b[0m");
    for &(i, total_ms) in &by_total[..top_n] {
        let (name, t) = &entries[i];
        let (dom_name, dom_ms) = dominant_stage(t);
        println!(
            "  {:<40} {:>4}ms  ({dom_name} {dom_ms}ms)",
            truncate_name(name, 40),
            total_ms,
        );
    }
}

/// Return the name and duration (ms) of the dominant (longest) stage.
fn dominant_stage(t: &CaptureTimings) -> (&'static str, u128) {
    STAGE_NAMES
        .iter()
        .zip(stage_durations(t))
        .map(|(&name, d)| (name, d.as_millis()))
        .max_by_key(|&(_, ms)| ms)
        .unwrap_or(("unknown", 0))
}

/// Truncate a snapshot name to `max` chars, keeping the tail (the unique part).
fn truncate_name(name: &str, max: usize) -> String {
    let len = name.chars().count();
    if len <= max {
        name.to_string()
    } else {
        let skip = len - (max - 1);
        let truncated: String = name.chars().skip(skip).collect();
        format!("…{truncated}")
    }
}
