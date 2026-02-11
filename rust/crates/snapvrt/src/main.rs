mod capture;
mod cdp;
mod cli;
mod commands;
mod compare;
mod config;
mod report;
mod store;
mod storybook;

use clap::Parser;
use config::{CliOverrides, ResolvedRunConfig};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("snapvrt=info")),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let cli = cli::Cli::parse();

    match cli.command {
        cli::Command::Init { url, force } => {
            commands::init(&url, force)?;
        }
        cli::Command::Review { open } => {
            commands::review(open)?;
        }
        cli::Command::Test {
            url,
            filter,
            threshold,
            timings,
            prune,
            capture,
        } => {
            let overrides = CliOverrides {
                url,
                threshold,
                capture,
            };
            let config = ResolvedRunConfig::new(overrides)?;
            let code = commands::test(config, filter.as_deref(), timings, prune).await?;
            std::process::exit(code);
        }
        cli::Command::Prune {
            url,
            dry_run,
            yes,
            capture,
        } => {
            let overrides = CliOverrides {
                url,
                threshold: None,
                capture,
            };
            let config = ResolvedRunConfig::new(overrides)?;
            commands::prune(config, dry_run, yes).await?;
        }
        cli::Command::Approve {
            filter,
            new,
            failed,
            all,
        } => {
            commands::approve(filter.as_deref(), new, failed, all)?;
        }
        cli::Command::Update {
            url,
            filter,
            timings,
            capture,
        } => {
            let overrides = CliOverrides {
                url,
                threshold: None,
                capture,
            };
            let config = ResolvedRunConfig::new(overrides)?;
            commands::update(config, filter.as_deref(), timings).await?;
        }
    }

    Ok(())
}
