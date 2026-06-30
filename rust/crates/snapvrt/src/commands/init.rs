use anyhow::{Result, bail};

use crate::cli::InitType;
use crate::config;
use crate::config::InitSourceType;

/// `snapvrt init` — create .snapvrt/config.toml.
pub fn init(init_type: InitType, url: &str, include: &str, force: bool) -> Result<()> {
    if !force && config::config_file_exists() {
        bail!(".snapvrt/config.toml already exists (use --force to overwrite)");
    }

    let source = match init_type {
        InitType::Storybook => InitSourceType::Storybook {
            url: url.to_string(),
        },
        InitType::Typst => InitSourceType::Typst {
            include: include.to_string(),
        },
        InitType::Pages => InitSourceType::Pages {
            url: url.to_string(),
        },
    };

    config::write_template(&source)?;
    config::write_gitignore(force)?;

    let verb = if force { "Regenerated" } else { "Created" };
    println!("{verb} .snapvrt/config.toml");
    match init_type {
        InitType::Storybook => println!("  source type = storybook, url = {url}"),
        InitType::Typst => println!("  source type = typst, include = {include}"),
        InitType::Pages => println!("  source type = pages, base_url = {url}"),
    }
    Ok(())
}
