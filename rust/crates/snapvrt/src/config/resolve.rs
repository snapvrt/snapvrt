use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};

use super::capture::CaptureConfig;
use super::{SourceConfig, Viewport, load, validate_threshold};

/// Values extracted from the CLI that participate in the merge.
pub struct CliOverrides {
    pub url: Option<String>,
    pub threshold: Option<f64>,
    pub capture: CaptureConfig,
}

/// Source-specific resolved configuration.
pub enum ResolvedSource {
    Storybook {
        url: String,
    },
    Typst {
        root: PathBuf,
        include: Vec<String>,
        scale: f32,
        pdf: bool,
        font_paths: Vec<String>,
    },
    Pages {
        base_url: String,
        pages: Vec<String>,
    },
}

/// One resolved source with its name and viewport subset.
pub struct ResolvedSourceEntry {
    pub name: String,
    pub source: ResolvedSource,
    pub viewports: BTreeMap<String, Viewport>,
}

/// Fully resolved config after CLI > env > file > defaults merge.
pub struct ResolvedRunConfig {
    pub sources: Vec<ResolvedSourceEntry>,
    pub capture: CaptureConfig,
    pub diff_threshold: f64,
}

impl ResolvedRunConfig {
    pub fn new(cli: CliOverrides) -> Result<Self> {
        // 1. File layer
        let file_config = load().context("Run `snapvrt init` first")?;

        // 2. Env layer
        let env_url = std::env::var("SNAPVRT_STORYBOOK_URL").ok();
        let env_threshold: Option<f64> = std::env::var("SNAPVRT_DIFF_THRESHOLD")
            .ok()
            .map(|v| v.parse::<f64>())
            .transpose()
            .context("SNAPVRT_DIFF_THRESHOLD must be a valid float")?;

        // 3. Diff threshold: CLI > env > file
        let diff_threshold = cli
            .threshold
            .or(env_threshold)
            .unwrap_or(file_config.diff.threshold);
        validate_threshold(diff_threshold).map_err(|e| anyhow::anyhow!("{e}"))?;

        // 4. Merge capture: file base, then CLI overlay
        let mut capture = file_config.capture;
        capture.merge(&cli.capture);

        // 5. Resolve all sources
        let mut sources = Vec::new();
        for (source_name, source_config) in &file_config.source {
            let source = match source_config {
                SourceConfig::Storybook { url, .. } => {
                    let storybook_url = cli
                        .url
                        .clone()
                        .or_else(|| env_url.clone())
                        .unwrap_or_else(|| url.clone());
                    ResolvedSource::Storybook { url: storybook_url }
                }
                SourceConfig::Typst {
                    root,
                    include,
                    scale,
                    pdf,
                    font_paths,
                } => ResolvedSource::Typst {
                    root: PathBuf::from(root),
                    include: include.clone(),
                    scale: *scale,
                    pdf: *pdf,
                    font_paths: font_paths.clone(),
                },
                SourceConfig::Pages {
                    base_url, pages, ..
                } => ResolvedSource::Pages {
                    base_url: cli
                        .url
                        .clone()
                        .or_else(|| env_url.clone())
                        .unwrap_or_else(|| base_url.clone()),
                    pages: pages.clone(),
                },
            };

            // Resolve viewports: if source specifies a subset, filter; otherwise use all
            let viewports = match source_config.viewports() {
                Some(selected) => {
                    let mut filtered = BTreeMap::new();
                    for name in selected {
                        if let Some(vp) = file_config.viewport.get(name) {
                            filtered.insert(name.clone(), vp.clone());
                        }
                    }
                    filtered
                }
                None => file_config.viewport.clone(),
            };

            sources.push(ResolvedSourceEntry {
                name: source_name.clone(),
                source,
                viewports,
            });
        }

        Ok(Self {
            sources,
            capture,
            diff_threshold,
        })
    }
}
