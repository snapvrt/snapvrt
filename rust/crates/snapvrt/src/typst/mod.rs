use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tracing::{debug, warn};

/// A data fixture for a template.
#[derive(Debug, Clone)]
pub struct TypstFixture {
    /// Fixture name (e.g. "default", "many-items") — used in snapshot IDs.
    pub name: String,
    /// Path to the JSON data file.
    pub data_path: PathBuf,
}

/// A discovered Typst template ready for rendering.
#[derive(Debug, Clone)]
pub struct TypstTemplate {
    /// Path relative to the working directory (e.g. "typst-templates/test/hello.typ").
    pub path: PathBuf,
    /// Stem used for snapshot IDs (e.g. "typst-templates/test/hello").
    pub stem: String,
    /// Data fixtures. Empty = self-contained template.
    pub fixtures: Vec<TypstFixture>,
}

/// Discover .typ files matching the given glob patterns.
///
/// For each template `foo.typ`, checks if `foo.fixtures/` directory exists.
/// If yes, each `.json` file inside becomes a fixture variant.
/// If no, the template is treated as self-contained (no fixtures).
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
                continue;
            }
            let stem = path.with_extension("").to_string_lossy().into_owned();

            // Check for <template>.fixtures/ directory
            let fixtures_dir = path.with_extension("fixtures");
            let fixtures = if fixtures_dir.is_dir() {
                discover_fixtures(&fixtures_dir)?
            } else {
                vec![]
            };

            templates.push(TypstTemplate {
                path,
                stem,
                fixtures,
            });
        }
    }

    templates.sort_by(|a, b| a.stem.cmp(&b.stem));
    Ok(templates)
}

/// Discover .json fixture files in a fixtures directory.
fn discover_fixtures(dir: &Path) -> Result<Vec<TypstFixture>> {
    let pattern = dir.join("*.json");
    let pattern_str = pattern.to_string_lossy();
    let paths =
        glob::glob(&pattern_str).with_context(|| format!("Invalid fixture glob: {pattern_str}"))?;

    let mut fixtures = Vec::new();
    for entry in paths {
        let data_path = entry.with_context(|| "Error reading fixture glob")?;
        if !data_path.is_file() {
            continue;
        }
        let name = data_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        fixtures.push(TypstFixture { name, data_path });
    }
    fixtures.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(fixtures)
}

/// Rendered page from a Typst template.
pub struct RenderedPage {
    /// Page number (1-based).
    pub page: usize,
    /// PNG bytes.
    pub png: Vec<u8>,
}

/// RAII guard that removes a temporary `data.json` file on drop.
struct DataJsonGuard {
    path: PathBuf,
}

impl Drop for DataJsonGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Options for a single compile invocation.
pub struct CompileOptions<'a> {
    pub root: &'a Path,
    pub template: &'a Path,
    pub fixture: Option<&'a TypstFixture>,
    pub scale: f32,
    /// If set, also compile a PDF and write it to this path.
    pub pdf_path: Option<PathBuf>,
    /// Additional font search paths passed as `--font-path` to typst.
    pub font_paths: &'a [String],
}

/// Compile a single Typst template to PNG pages.
///
/// If a fixture is provided, its JSON content is temporarily written as
/// `data.json` next to the template (cleaned up via Drop guard).
///
/// Optionally also compiles a PDF for debugging.
pub async fn compile(opts: &CompileOptions<'_>) -> Result<Vec<RenderedPage>> {
    // If fixture provided, write data.json next to the template
    let _guard = if let Some(fixture) = opts.fixture {
        let data_json_path = opts
            .template
            .parent()
            .context("Template has no parent directory")?
            .join("data.json");
        std::fs::copy(&fixture.data_path, &data_json_path).with_context(|| {
            format!(
                "Failed to copy fixture {} → {}",
                fixture.data_path.display(),
                data_json_path.display()
            )
        })?;
        debug!(
            fixture = %fixture.name,
            data_json = %data_json_path.display(),
            "wrote data.json for fixture"
        );
        Some(DataJsonGuard {
            path: data_json_path,
        })
    } else {
        None
    };

    let pages = compile_png(opts.root, opts.template, opts.scale, opts.font_paths).await?;

    // Optionally compile PDF for debugging
    if let Some(ref pdf_path) = opts.pdf_path {
        compile_pdf(opts.root, opts.template, pdf_path, opts.font_paths).await?;
    }

    Ok(pages)
}

/// Compile a template to PNG pages in a temp directory.
async fn compile_png(
    root: &Path,
    template: &Path,
    scale: f32,
    font_paths: &[String],
) -> Result<Vec<RenderedPage>> {
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
        .arg(root);
    for fp in font_paths {
        cmd.arg("--font-path").arg(fp);
    }
    cmd.arg(template).arg(&output_pattern);

    debug!(
        template = %template.display(),
        ppi,
        "compiling typst template (png)"
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

    // Show warnings (e.g. missing fonts) even on successful compilation
    if !output.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            template = %template.display(),
            "typst compile warnings:\n{}",
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
            Err(_) => break,
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

/// Compile a template to PDF and write it to the given path.
async fn compile_pdf(
    root: &Path,
    template: &Path,
    pdf_path: &Path,
    font_paths: &[String],
) -> Result<()> {
    if let Some(parent) = pdf_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let mut cmd = tokio::process::Command::new("typst");
    cmd.arg("compile")
        .arg("--format")
        .arg("pdf")
        .arg("--root")
        .arg(root);
    for fp in font_paths {
        cmd.arg("--font-path").arg(fp);
    }
    cmd.arg(template).arg(pdf_path);

    debug!(
        template = %template.display(),
        pdf = %pdf_path.display(),
        "compiling typst template (pdf)"
    );

    let output = cmd.output().await.with_context(|| {
        format!(
            "Failed to run `typst compile` (pdf) for {}",
            template.display()
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(
            template = %template.display(),
            "PDF generation failed: {}",
            stderr.trim()
        );
    }

    Ok(())
}
