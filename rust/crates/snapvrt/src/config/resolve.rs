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

/// Fully resolved config after CLI > env > file > defaults merge.
pub struct ResolvedRunConfig {
    pub source: ResolvedSource,
    pub capture: CaptureConfig,
    pub diff_threshold: f64,
    pub viewports: BTreeMap<String, Viewport>,
    /// Source name (from `[source.<name>]` map key), used as top-level
    /// directory in the snapshot hierarchy.
    pub source_name: String,
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

        // 3. Extract the single source (multi-source is future work)
        let (source_name, source_config) = file_config
            .source
            .iter()
            .next()
            .context("No sources configured — add a [source.<name>] section")?;
        let source_name = source_name.to_owned();

        // 4. Resolve source-specific config
        let source = match source_config {
            SourceConfig::Storybook { url, .. } => {
                let storybook_url = cli.url.or(env_url).unwrap_or_else(|| url.clone());
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
                base_url: cli.url.or(env_url).unwrap_or_else(|| base_url.clone()),
                pages: pages.clone(),
            },
        };

        // 5. Diff threshold: CLI > env > file
        let diff_threshold = cli
            .threshold
            .or(env_threshold)
            .unwrap_or(file_config.diff.threshold);
        validate_threshold(diff_threshold).map_err(|e| anyhow::anyhow!("{e}"))?;

        // 6. Merge capture: file base, then CLI overlay
        let mut capture = file_config.capture;
        capture.merge(&cli.capture);

        // 7. Resolve viewports: if source specifies a subset, filter; otherwise use all
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
            None => file_config.viewport,
        };

        Ok(Self {
            source,
            capture,
            diff_threshold,
            viewports,
            source_name,
        })
    }
}
