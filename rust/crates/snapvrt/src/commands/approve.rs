use anyhow::{Result, bail};

use crate::store;
use crate::storybook::normalize_for_filter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    New,
    Failed,
}

pub fn approve(filter: Option<&str>, new_only: bool, failed_only: bool, all: bool) -> Result<()> {
    let (new_only, failed_only) = if all {
        (false, false)
    } else {
        (new_only, failed_only)
    };
    let ids = store::list_current_ids();
    if ids.is_empty() {
        println!("Nothing to approve — current/ is empty.");
        return Ok(());
    }

    // Classify each id.
    let classified: Vec<(&str, Kind)> = ids
        .iter()
        .map(|id| {
            let kind = if store::has_difference(id) {
                Kind::Failed
            } else {
                Kind::New
            };
            (id.as_str(), kind)
        })
        .collect();

    // Filter by kind.
    let kind_filtered: Vec<(&str, Kind)> = classified
        .into_iter()
        .filter(|(_, kind)| {
            if new_only {
                *kind == Kind::New
            } else if failed_only {
                *kind == Kind::Failed
            } else {
                true // --all or default
            }
        })
        .collect();

    // Filter by pattern (strip .png suffix — user may copy from HTML review page).
    // Normalize spaces/underscores so both terminal output and raw names work.
    let filtered: Vec<(&str, Kind)> = kind_filtered
        .into_iter()
        .filter(|(id, _)| {
            filter
                .map(|pat| {
                    let pat = pat.strip_suffix(".png").unwrap_or(pat);
                    normalize_for_filter(id).contains(&normalize_for_filter(pat))
                })
                .unwrap_or(true)
        })
        .collect();

    if filtered.is_empty() {
        println!("No snapshots matched the given filters.");
        return Ok(());
    }

    let mut count_new = 0usize;
    let mut count_failed = 0usize;

    for (id, kind) in &filtered {
        let bytes = store::read_current(id);
        match bytes {
            Some(png) => {
                store::write_reference(id, &png)?;
                let label = match kind {
                    Kind::Failed => {
                        count_failed += 1;
                        "\x1b[31mFAIL\x1b[0m"
                    }
                    Kind::New => {
                        count_new += 1;
                        "\x1b[33m NEW\x1b[0m"
                    }
                };
                println!("  Approved  {label}  {id}");
            }
            None => {
                bail!("Could not read current/{id}.png");
            }
        }
    }

    let total = count_new + count_failed;
    println!();
    println!("{total} snapshot(s) approved ({count_new} new, {count_failed} failed).");

    Ok(())
}
