use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use image::ImageReader;

use poc_image_diff::{
    DiffEngine, engine_dify::DifyEngine, engine_pixel::PixelEngine, engine_ssim::SsimEngine,
};

#[derive(Parser)]
#[command(about = "Compare image diff engines side-by-side")]
struct Cli {
    /// Left / reference image (PNG)
    #[arg(long)]
    left: PathBuf,

    /// Right / current image (PNG)
    #[arg(long)]
    right: PathBuf,

    /// Output directory for diff images
    #[arg(long, default_value = ".")]
    output: PathBuf,
}

struct EngineRun {
    name: String,
    score: f64,
    diff_pixels: u64,
    total_pixels: u64,
    elapsed_ms: f64,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load images
    let left = ImageReader::open(&cli.left)
        .with_context(|| format!("failed to open left image: {}", cli.left.display()))?
        .decode()
        .context("failed to decode left image")?
        .to_rgba8();

    let right = ImageReader::open(&cli.right)
        .with_context(|| format!("failed to open right image: {}", cli.right.display()))?
        .decode()
        .context("failed to decode right image")?
        .to_rgba8();

    println!(
        "Left:  {} ({}x{})",
        cli.left.display(),
        left.width(),
        left.height()
    );
    println!(
        "Right: {} ({}x{})",
        cli.right.display(),
        right.width(),
        right.height()
    );
    println!();

    // Ensure output directory exists
    std::fs::create_dir_all(&cli.output)
        .with_context(|| format!("failed to create output dir: {}", cli.output.display()))?;

    let engines: Vec<Box<dyn DiffEngine>> = vec![
        Box::new(DifyEngine::default()),
        Box::new(SsimEngine),
        Box::new(PixelEngine::default()),
    ];

    let mut runs = Vec::new();

    for engine in &engines {
        let start = Instant::now();
        let result = engine.diff(&left, &right)?;
        let elapsed = start.elapsed();

        // Save diff image if produced
        if let Some(ref diff_img) = result.diff_image {
            let out_path = cli.output.join(format!("diff-{}.png", engine.name()));
            diff_img
                .save(&out_path)
                .with_context(|| format!("failed to save diff image: {}", out_path.display()))?;
            println!("  saved {}", out_path.display());
        }

        runs.push(EngineRun {
            name: engine.name().to_string(),
            score: result.score,
            diff_pixels: result.diff_pixels,
            total_pixels: result.total_pixels,
            elapsed_ms: elapsed.as_secs_f64() * 1000.0,
        });
    }

    // Print comparison table
    println!();
    println!(
        "{:<10} {:>10} {:>14} {:>14} {:>10}",
        "Engine", "Score", "Diff Pixels", "Total Pixels", "Time (ms)"
    );
    println!("{}", "-".repeat(62));
    for run in &runs {
        println!(
            "{:<10} {:>10.6} {:>14} {:>14} {:>10.2}",
            run.name, run.score, run.diff_pixels, run.total_pixels, run.elapsed_ms
        );
    }

    Ok(())
}
