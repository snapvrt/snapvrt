use std::path::Path;

use anyhow::{Context, Result};

use super::{CONFIG_DIR, CONFIG_FILE};

/// Hand-crafted config template with commented-out keys.
/// Used by `snapvrt init` instead of `toml::to_string_pretty()` so that
/// users can see the available knobs without uncommenting section headers.
const STORYBOOK_TEMPLATE: &str = r#"[source.storybook]
type = "storybook"
url = "{url}"
# viewports = ["laptop"]           # optional: omit = use all defined viewports

[viewport.laptop]
width = 1366
height = 768

# ─────────────────────────────────────────────────────────
# Capture pipeline — all fields optional.
# ─────────────────────────────────────────────────────────
[capture]
# screenshot = "stable"             # "stable" | "single" (single is faster)
# stability_attempts = 3
# stability_delay_ms = 100
# parallel = 4                      # concurrent browser tabs
# chrome_url = "http://localhost:9222"  # remote Chrome (e.g. Docker)

# ─────────────────────────────────────────────────────────
# Comparison — all fields optional.
# ─────────────────────────────────────────────────────────
[diff]
# threshold = 0.0                   # max allowed diff score (0.0 = exact, 0.01 = 1%)
"#;

const TYPST_TEMPLATE: &str = r#"[source.typst]
type = "typst"
root = "."
include = ["{include}"]
# scale = 2.0                      # PNG scale factor (default: 2.0 → 144 PPI)
# pdf = false                      # also generate PDFs in .snapvrt/pdf/ for debugging

# ─────────────────────────────────────────────────────────
# Comparison — all fields optional.
# ─────────────────────────────────────────────────────────
[diff]
# threshold = 0.0                   # max allowed diff score (0.0 = exact, 0.01 = 1%)
"#;

pub fn config_file_exists() -> bool {
    Path::new(CONFIG_DIR).join(CONFIG_FILE).exists()
}

pub fn write_gitignore(force: bool) -> Result<()> {
    let path = Path::new(CONFIG_DIR).join(".gitignore");
    if !force && path.exists() {
        return Ok(());
    }
    std::fs::write(&path, "current/\ndifference/\npdf/\nreport.html\n")
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

#[allow(dead_code)]
pub fn save(config: &super::Config) -> Result<()> {
    let dir = Path::new(CONFIG_DIR);
    std::fs::create_dir_all(dir).context("Failed to create .snapvrt directory")?;
    let path = dir.join(CONFIG_FILE);
    let content = toml::to_string_pretty(config).context("Failed to serialize config")?;
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Source type for init.
pub enum InitSourceType {
    Storybook { url: String },
    Typst { include: String },
}

/// Write the hand-crafted config template (with commented-out sections).
/// Used by `snapvrt init` instead of `save()`.
pub fn write_template(source: &InitSourceType) -> Result<()> {
    let dir = Path::new(CONFIG_DIR);
    std::fs::create_dir_all(dir).context("Failed to create .snapvrt directory")?;
    let path = dir.join(CONFIG_FILE);
    let content = match source {
        InitSourceType::Storybook { url } => STORYBOOK_TEMPLATE.replace("{url}", url),
        InitSourceType::Typst { include } => TYPST_TEMPLATE.replace("{include}", include),
    };
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}
