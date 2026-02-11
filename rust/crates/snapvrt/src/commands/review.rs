use anyhow::{Context, Result};

use crate::report::html;
use crate::store;

fn open_in_browser(path: &std::path::Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "linux")]
    let cmd = "xdg-open";
    #[cfg(target_os = "windows")]
    let cmd = "start";

    std::process::Command::new(cmd)
        .arg(path)
        .spawn()
        .context("Failed to open report in browser")?;
    Ok(())
}

/// `snapvrt review` — generate static HTML report.
pub fn review(open: bool) -> Result<()> {
    let summary = html::generate()?;
    println!("Report written to {summary}");

    if open {
        let report_path = std::path::Path::new(store::BASE_DIR).join("report.html");
        let path =
            std::fs::canonicalize(&report_path).unwrap_or_else(|_| report_path.to_path_buf());
        open_in_browser(&path)?;
    }

    Ok(())
}
