use clap::{Parser, Subcommand};

use crate::config;
use crate::config::CaptureConfig;

fn parse_threshold(s: &str) -> Result<f64, String> {
    let v: f64 = s.parse().map_err(|e| format!("{e}"))?;
    config::validate_threshold(v)
}

#[derive(Parser)]
#[command(
    name = "snapvrt",
    about = "Visual regression testing for UI components"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create .snapvrt/config.toml with default settings
    Init {
        /// Storybook URL
        #[arg(long, default_value = "http://localhost:6006")]
        url: String,
        /// Overwrite existing config and gitignore
        #[arg(long, short = 'f')]
        force: bool,
    },

    /// Discover, capture, compare, and report visual differences (exit 0/1)
    Test {
        /// Storybook URL (overrides config)
        #[arg(long)]
        url: Option<String>,
        /// Only run snapshots whose name contains PATTERN (case-insensitive)
        #[arg(long, short = 'f')]
        filter: Option<String>,
        /// Max allowed diff score (0.0–1.0). Snapshots within threshold pass.
        #[arg(long, value_parser = parse_threshold)]
        threshold: Option<f64>,
        /// Print per-snapshot timing breakdown table
        #[arg(long)]
        timings: bool,
        /// Delete orphaned reference snapshots that no longer match any story
        #[arg(long)]
        prune: bool,
        #[command(flatten)]
        capture: CaptureConfig,
    },

    /// Generate a visual review report (static HTML)
    Review {
        /// Open the report in the default browser
        #[arg(long)]
        open: bool,
    },

    /// Promote current/ snapshots to reference/ without re-capturing
    Approve {
        /// Only approve snapshots whose name contains PATTERN (case-insensitive)
        #[arg(long, short = 'f')]
        filter: Option<String>,
        /// Only approve new snapshots (no prior reference)
        #[arg(long)]
        new: bool,
        /// Only approve failed snapshots (have a diff)
        #[arg(long)]
        failed: bool,
        /// Approve all pending snapshots (default when no kind flags)
        #[arg(long)]
        all: bool,
    },

    /// Delete orphaned reference snapshots that no longer match any story
    Prune {
        /// Storybook URL (overrides config)
        #[arg(long)]
        url: Option<String>,
        /// Show what would be deleted without deleting
        #[arg(long)]
        dry_run: bool,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
        #[command(flatten)]
        capture: CaptureConfig,
    },

    /// Discover, capture, and save as reference snapshots
    Update {
        /// Storybook URL (overrides config)
        #[arg(long)]
        url: Option<String>,
        /// Only run snapshots whose name contains PATTERN (case-insensitive)
        #[arg(long, short = 'f')]
        filter: Option<String>,
        /// Print per-snapshot timing breakdown table
        #[arg(long)]
        timings: bool,
        #[command(flatten)]
        capture: CaptureConfig,
    },
}
