use std::path::Path;

use anyhow::{Context, Result};

use super::{CONFIG_DIR, CONFIG_FILE};

/// Hand-crafted config template with commented-out keys.
/// Used by `snapvrt init` instead of `toml::to_string_pretty()` so that
/// users can see the available knobs without uncommenting section headers.
const CONFIG_TEMPLATE: &str = r#"[source.storybook]
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

pub fn config_file_exists() -> bool {
    Path::new(CONFIG_DIR).join(CONFIG_FILE).exists()
}

pub fn write_gitignore(force: bool) -> Result<()> {
    let path = Path::new(CONFIG_DIR).join(".gitignore");
    if !force && path.exists() {
        return Ok(());
    }
    std::fs::write(&path, "current/\ndifference/\nreport.html\n")
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

/// Write the hand-crafted config template (with commented-out sections).
/// Used by `snapvrt init` instead of `save()`.
pub fn write_template(url: &str) -> Result<()> {
    let dir = Path::new(CONFIG_DIR);
    std::fs::create_dir_all(dir).context("Failed to create .snapvrt directory")?;
    let path = dir.join(CONFIG_FILE);
    let content = CONFIG_TEMPLATE.replace("{url}", url);
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}
