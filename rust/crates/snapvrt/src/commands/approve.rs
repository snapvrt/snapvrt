use anyhow::{Result, bail};

use crate::config::SourceFilter;
use crate::store;
use crate::storybook::snapshot_name_matches;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    New,
    Failed,
    Pass,
}

pub fn approve(
    filter: Option<&str>,
    new_only: bool,
    failed_only: bool,
    all: bool,
    source_filter: &SourceFilter,
) -> Result<()> {
    let (new_only, failed_only) = if all {
        (false, false)
    } else {
        (new_only, failed_only)
    };
    let all_ids = store::list_current_ids();
    if all_ids.is_empty() {
        println!("Nothing to approve — current/ is empty.");
        return Ok(());
    }
    // Restrict to the selected sources before classifying/pattern-matching; an
    // empty result here falls through to the "no snapshots matched" message.
    let ids: Vec<String> = all_ids
        .into_iter()
        .filter(|id| source_filter.matches_id(id))
        .collect();

    // Classify each id.
    let classified: Vec<(&str, Kind)> = ids
        .iter()
        .map(|id| {
            let kind = if store::has_difference(id) {
                Kind::Failed
            } else if store::has_reference(id) {
                Kind::Pass
            } else {
                Kind::New
            };
            (id.as_str(), kind)
        })
        .collect();

    // Filter by kind — skip Pass (reference matches current, nothing to approve).
    let kind_filtered: Vec<(&str, Kind)> = classified
        .into_iter()
        .filter(|(_, kind)| *kind != Kind::Pass)
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

    let filtered: Vec<(&str, Kind)> = kind_filtered
        .into_iter()
        .filter(|(id, _)| {
            filter
                .map(|pat| snapshot_name_matches(id, pat))
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
                    Kind::Pass => unreachable!("Pass entries are filtered out"),
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
