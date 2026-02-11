use std::io::Write;

use anyhow::Result;

use crate::capture::CapturePlan;
use crate::config::ResolvedRunConfig;
use crate::store;

/// `snapvrt prune` — find and delete orphaned reference snapshots.
pub async fn prune(config: ResolvedRunConfig, dry_run: bool, yes: bool) -> Result<()> {
    let run = CapturePlan::plan(&config, None).await?;
    let planned_ids: std::collections::BTreeSet<String> = run.job_names().into_iter().collect();
    let reference_ids = store::list_reference_ids();

    let orphans: Vec<&String> = reference_ids.difference(&planned_ids).collect();

    if orphans.is_empty() {
        println!("No orphaned references found.");
        return Ok(());
    }

    println!("Orphaned references ({}):", orphans.len());
    for id in &orphans {
        println!("  {id}");
    }
    println!();

    if dry_run {
        println!("Dry run — no files deleted.");
        return Ok(());
    }

    if !yes {
        print!("Delete {} reference(s)? [y/N] ", orphans.len());
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    for id in &orphans {
        store::remove_reference(id);
    }
    println!("Deleted {} orphaned reference(s).", orphans.len());

    Ok(())
}
