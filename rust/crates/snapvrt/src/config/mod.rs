pub mod capture;
pub mod resolve;
pub mod template;

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub use self::capture::CaptureConfig;
pub use self::resolve::{CliOverrides, ResolvedRunConfig};
pub use self::template::{config_file_exists, write_gitignore, write_template};

pub(crate) const CONFIG_DIR: &str = ".snapvrt";
const CONFIG_FILE: &str = "config.toml";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiffConfig {
    /// Maximum allowed diff score (0.0-1.0). Snapshots with score <= threshold pass.
    #[serde(default)]
    pub threshold: f64,
}

pub fn validate_threshold(v: f64) -> Result<f64, String> {
    if !(0.0..=1.0).contains(&v) {
        return Err(format!("threshold must be between 0.0 and 1.0, got {v}"));
    }
    Ok(v)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub source: BTreeMap<String, SourceConfig>,
    #[serde(default = "default_viewports")]
    pub viewport: BTreeMap<String, Viewport>,
    #[serde(default, rename = "capture")]
    pub capture: CaptureConfig,
    #[serde(default)]
    pub diff: DiffConfig,
}

impl Config {
    /// Validate semantic constraints that serde cannot express.
    fn validate(&self) -> Result<()> {
        if self.source.is_empty() {
            bail!(
                "No sources configured. Add a source section, e.g.:\n\n  \
                 [source.storybook]\n  \
                 type = \"storybook\"\n  \
                 url = \"http://localhost:6006\""
            );
        }

        if self.viewport.is_empty() {
            bail!(
                "No viewports configured. Add a viewport section, e.g.:\n\n  \
                 [viewport.laptop]\n  \
                 width = 1366\n  \
                 height = 768"
            );
        }

        for (name, vp) in &self.viewport {
            if vp.width == 0 || vp.height == 0 {
                bail!(
                    "Viewport '{name}' has invalid dimensions ({}x{}). \
                     Both width and height must be > 0",
                    vp.width,
                    vp.height,
                );
            }
        }

        for (source_name, source) in &self.source {
            if let Some(refs) = source.viewports() {
                for vp_ref in refs {
                    if !self.viewport.contains_key(vp_ref) {
                        let defined: Vec<&str> = self.viewport.keys().map(|k| k.as_str()).collect();
                        bail!(
                            "Source '{source_name}' references viewport '{vp_ref}', \
                             but it is not defined. Defined viewports: {}",
                            defined.join(", "),
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SourceConfig {
    #[serde(rename = "storybook")]
    Storybook {
        url: String,
        #[serde(default)]
        viewports: Option<Vec<String>>,
    },
}

impl SourceConfig {
    pub fn url(&self) -> &str {
        match self {
            Self::Storybook { url, .. } => url,
        }
    }

    pub fn viewports(&self) -> Option<&[String]> {
        match self {
            Self::Storybook { viewports, .. } => viewports.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

fn default_viewports() -> BTreeMap<String, Viewport> {
    let mut m = BTreeMap::new();
    m.insert(
        "laptop".to_string(),
        Viewport {
            width: 1366,
            height: 768,
        },
    );
    m
}

pub fn load() -> Result<Config> {
    let path = Path::new(CONFIG_DIR).join(CONFIG_FILE);
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let config: Config =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;
    validate_threshold(config.diff.threshold).map_err(|e| anyhow::anyhow!("diff.{e}"))?;
    config.validate()?;
    Ok(config)
}
