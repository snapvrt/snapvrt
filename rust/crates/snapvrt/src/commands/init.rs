use anyhow::{Result, bail};

use crate::config;

/// `snapvrt init` — create .snapvrt/config.toml.
pub fn init(url: &str, force: bool) -> Result<()> {
    if !force && config::config_file_exists() {
        bail!(".snapvrt/config.toml already exists (use --force to overwrite)");
    }

    config::write_template(url)?;
    config::write_gitignore(force)?;

    let verb = if force { "Regenerated" } else { "Created" };
    println!("{verb} .snapvrt/config.toml");
    println!("  source.storybook.url = {url}");
    Ok(())
}
