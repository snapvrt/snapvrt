use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tracing::debug;

/// A discovered Typst template ready for rendering.
#[derive(Debug, Clone)]
pub struct TypstTemplate {
    /// Path relative to the working directory (e.g. "typst-templates/test/hello.typ").
    pub path: PathBuf,
    /// Stem used for snapshot IDs (e.g. "typst-templates/test/hello").
    pub stem: String,
}

/// Discover .typ files matching the given glob patterns.
pub fn discover(include: &[String]) -> Result<Vec<TypstTemplate>> {
    let mut templates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for pattern in include {
        let paths =
            glob::glob(pattern).with_context(|| format!("Invalid glob pattern: {pattern}"))?;

        for entry in paths {
            let path = entry.with_context(|| format!("Error reading glob result for {pattern}"))?;
            if !path.is_file() || path.extension().is_none_or(|e| e != "typ") {
                continue;
            }
            if !seen.insert(path.clone()) {
                continue; // deduplicate
            }
            let stem = path.with_extension("").to_string_lossy().into_owned();
            templates.push(TypstTemplate { path, stem });
        }
    }

    templates.sort_by(|a, b| a.stem.cmp(&b.stem));
    Ok(templates)
}

/// Rendered page from a Typst template.
pub struct RenderedPage {
    /// Page number (1-based).
    pub page: usize,
    /// PNG bytes.
    pub png: Vec<u8>,
}

/// Compile a single Typst template to PNG pages.
///
/// Uses `typst compile` with `--format png` and a `{p}` placeholder for
/// multi-page output. Returns one `RenderedPage` per page.
pub async fn compile(root: &Path, template: &Path, scale: f32) -> Result<Vec<RenderedPage>> {
    let ppi = (scale * 72.0).round() as u32;
    let temp_dir = tempfile::tempdir().context("Failed to create temp dir for typst output")?;
    let output_pattern = temp_dir.path().join("{p}.png");

    let mut cmd = tokio::process::Command::new("typst");
    cmd.arg("compile")
        .arg("--format")
        .arg("png")
        .arg("--ppi")
        .arg(ppi.to_string())
        .arg("--root")
        .arg(root)
        .arg(template)
        .arg(&output_pattern);

    debug!(
        template = %template.display(),
        ppi,
        "compiling typst template"
    );

    let output = cmd
        .output()
        .await
        .with_context(|| format!("Failed to run `typst compile` for {}", template.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "typst compile failed for {}:\n{}",
            template.display(),
            stderr.trim()
        );
    }

    // Read all page PNGs (1.png, 2.png, 3.png, ...)
    let mut pages = Vec::new();
    for page_num in 1.. {
        let page_path = temp_dir.path().join(format!("{page_num}.png"));
        match std::fs::read(&page_path) {
            Ok(png) => {
                debug!(
                    template = %template.display(),
                    page = page_num,
                    bytes = png.len(),
                    "read page"
                );
                pages.push(RenderedPage {
                    page: page_num,
                    png,
                });
            }
            Err(_) => break, // no more pages
        }
    }

    if pages.is_empty() {
        bail!(
            "typst compile produced no output for {}",
            template.display()
        );
    }

    Ok(pages)
}
