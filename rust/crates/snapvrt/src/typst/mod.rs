use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tracing::{debug, warn};

use crate::config::TypstTemplateEntry;

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
pub fn discover(include: &[String], explicit: &[TypstTemplateEntry]) -> Result<Vec<TypstTemplate>> {
    let mut templates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Explicit `template → fixtures` entries first. Their fixtures dir may live
    // outside the sibling `<template>.fixtures/` location (e.g. a generated
    // `fixtures/<kind>/` tree), and an explicit entry wins over an include
    // glob's sibling discovery on overlap (processed first → marked seen).
    for entry in explicit {
        // Each `fixtures` spec is a directory (all its `*.json`), a file, or a
        // glob; a list of them lets a template pick an explicit subset without
        // duplicating shared datasets on disk.
        let mut fixtures = Vec::new();
        for spec in entry.fixtures.specs() {
            let pattern = if Path::new(spec).is_dir() {
                format!("{}/*.json", spec.trim_end_matches('/'))
            } else {
                spec.clone()
            };
            fixtures.extend(discover_fixtures_glob(&pattern)?);
        }
        fixtures.sort_by(|a, b| a.name.cmp(&b.name));
        if fixtures.is_empty() {
            bail!(
                "typst source: no fixtures matched {:?} for template glob `{}`",
                entry.fixtures.specs(),
                entry.path,
            );
        }
        let paths = glob::glob(&entry.path)
            .with_context(|| format!("Invalid glob pattern: {}", entry.path))?;
        let mut matched = false;
        for result in paths {
            let path =
                result.with_context(|| format!("Error reading glob result for {}", entry.path))?;
            if !path.is_file() || path.extension().is_none_or(|e| e != "typ") {
                continue;
            }
            matched = true;
            if !seen.insert(path.clone()) {
                continue;
            }
            let stem = path.with_extension("").to_string_lossy().into_owned();
            templates.push(TypstTemplate {
                path,
                stem,
                fixtures: fixtures.clone(),
            });
        }
        if !matched {
            warn!(
                "typst source: template glob `{}` matched no .typ files",
                entry.path
            );
        }
    }

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

/// Discover .json fixture files in a fixtures directory (its `*.json`).
fn discover_fixtures(dir: &Path) -> Result<Vec<TypstFixture>> {
    discover_fixtures_glob(&dir.join("*.json").to_string_lossy())
}

/// Discover .json fixture files matching a glob pattern. Each file's stem is the
/// fixture (snapshot-variant) name.
fn discover_fixtures_glob(pattern: &str) -> Result<Vec<TypstFixture>> {
    let paths = glob::glob(pattern).with_context(|| format!("Invalid fixture glob: {pattern}"))?;

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

/// Serializes staging into the shared `<root>/data/assets/` path. That virtual
/// path is root-relative, so every image-bearing template would otherwise stage
/// into the same directory and race a concurrent worker. Held for the whole
/// render (stage → compile → unstage).
static ASSETS_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// RAII guard: removes the staged asset files (and the now-empty `data/assets`
/// and `data` dirs) and releases [`ASSETS_LOCK`] on drop.
struct StagedAssets {
    staged: Vec<PathBuf>,
    /// Every virtual dir written into, e.g. `data/assets` and
    /// `data/employee-signatures`. Their shared `data` parent is removed last.
    dirs: Vec<PathBuf>,
    _lock: tokio::sync::MutexGuard<'static, ()>,
}

impl Drop for StagedAssets {
    fn drop(&mut self) {
        for path in &self.staged {
            let _ = std::fs::remove_file(path);
        }
        // Remove the dirs we created, only while empty — a pre-existing
        // populated `data/assets` is left intact.
        for dir in &self.dirs {
            let _ = std::fs::remove_dir(dir);
        }
        if let Some(data_dir) = self.dirs.first().and_then(|d| d.parent()) {
            let _ = std::fs::remove_dir(data_dir);
        }
    }
}

/// Mount a template's sibling asset directories under `<root>/data/`:
/// `images/` at `data/assets/<file>`, `employee-signatures/` at
/// `data/employee-signatures/<file>`, and `request-images/` at
/// `data/request-images/<file>`
/// for the duration of a render, mirroring the production print pipeline (and
/// the LIMS `typst-cli-tool`, which injects the same files at the
/// `/data/assets/<file>` virtual path). This lets a lab template reference assets
/// the console way — `#image("/data/assets/<file>")` — and have snapvrt resolve
/// them from the repo copy.
///
/// Returns `None` when the template has neither directory (nothing to mount).
/// Dotfiles and subdirectories are skipped, matching `load_local_assets`. Because
/// `data/assets` is a single root-relative path shared by every template, staging
/// is serialized behind [`ASSETS_LOCK`]; the returned guard holds the lock until
/// the render finishes, then unstages.
async fn stage_assets(root: &Path, template: &Path) -> Result<Option<StagedAssets>> {
    // (sibling directory, virtual subdirectory under `data/`). `images/` is the
    // console's asset convention; `employee-signatures/` carries the per-analyst
    // signature images a protocol references as
    // `/data/employee-signatures/<file>`; `request-images/` carries the photos
    // uploaded against a request, referenced as `/data/request-images/<file>`.
    // All three are injected by the production print pipeline and by `ltypst`,
    // so a fixture holding any of those path shapes renders the same way here.
    const MOUNTS: &[(&str, &str)] = &[
        ("images", "assets"),
        ("employee-signatures", "employee-signatures"),
        ("request-images", "request-images"),
    ];

    // Gather eligible files before taking the lock or touching the tree.
    let Some(parent) = template.parent() else {
        return Ok(None);
    };
    let mut sources: Vec<(PathBuf, String, &str)> = Vec::new();
    for (dir_name, virtual_name) in MOUNTS {
        let Ok(entries) = std::fs::read_dir(parent.join(dir_name)) else {
            continue; // no such dir — nothing to mount from it
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
                continue;
            };
            if name.starts_with('.') || !path.is_file() {
                continue;
            }
            sources.push((path, name, virtual_name));
        }
    }
    if sources.is_empty() {
        return Ok(None);
    }

    let lock = ASSETS_LOCK.lock().await;
    let data_dir = root.join("data");

    let mut staged = Vec::with_capacity(sources.len());
    let mut dirs: Vec<PathBuf> = Vec::new();
    for (src, name, virtual_name) in sources {
        let dir = data_dir.join(virtual_name);
        if !dirs.contains(&dir) {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create {}", dir.display()))?;
            dirs.push(dir.clone());
        }
        let dest = dir.join(&name);
        std::fs::copy(&src, &dest).with_context(|| {
            format!(
                "Failed to stage asset {} → {}",
                src.display(),
                dest.display()
            )
        })?;
        staged.push(dest);
    }

    debug!(
        template = %template.display(),
        count = staged.len(),
        dirs = dirs.len(),
        "mounted template assets under data/"
    );
    Ok(Some(StagedAssets {
        staged,
        dirs,
        _lock: lock,
    }))
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
    /// Additional local-package roots passed as `--package-path` to
    /// typst. Each must follow typst's standard
    /// `<dir>/<namespace>/<name>/<version>/` layout.
    pub package_paths: &'a [String],
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

    // Mount the template's sibling `images/` at `<root>/data/assets/` so
    // `#image("/data/assets/<file>")` references resolve, held across both the
    // PNG and PDF compiles below.
    let _assets = stage_assets(opts.root, opts.template).await?;

    let pages = compile_png(
        opts.root,
        opts.template,
        opts.scale,
        opts.font_paths,
        opts.package_paths,
    )
    .await?;

    // Optionally compile PDF for debugging
    if let Some(ref pdf_path) = opts.pdf_path {
        compile_pdf(
            opts.root,
            opts.template,
            pdf_path,
            opts.font_paths,
            opts.package_paths,
        )
        .await?;
    }

    Ok(pages)
}

/// Compile a template to PNG pages in a temp directory.
async fn compile_png(
    root: &Path,
    template: &Path,
    scale: f32,
    font_paths: &[String],
    package_paths: &[String],
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
    for pp in package_paths {
        cmd.arg("--package-path").arg(pp);
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
    package_paths: &[String],
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
    for pp in package_paths {
        cmd.arg("--package-path").arg(pp);
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
