use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result};

use crate::store;

const OUTPUT_FILE: &str = "report.html";

struct SnapshotRow {
    name: String,
    has_reference: bool,
    has_current: bool,
    has_difference: bool,
}

/// Recursively collect `.png` files as relative paths (including the `.png` extension).
fn list_png_relative(dir: &Path) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    collect_pngs(dir, dir, &mut result);
    result
}

fn collect_pngs(base: &Path, dir: &Path, out: &mut BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_pngs(base, &path, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
            && let Ok(rel) = path.strip_prefix(base)
        {
            out.insert(rel.to_string_lossy().into_owned());
        }
    }
}

fn collect_rows() -> Vec<SnapshotRow> {
    let base = Path::new(store::BASE_DIR);
    let reference = list_png_relative(&base.join(store::REFERENCE_DIR));
    let current = list_png_relative(&base.join(store::CURRENT_DIR));
    let difference = list_png_relative(&base.join(store::DIFFERENCE_DIR));

    let mut all_names = BTreeSet::new();
    all_names.extend(reference.iter().cloned());
    all_names.extend(current.iter().cloned());
    all_names.extend(difference.iter().cloned());

    all_names
        .into_iter()
        .map(|name| SnapshotRow {
            has_reference: reference.contains(&name),
            has_current: current.contains(&name),
            has_difference: difference.contains(&name),
            name,
        })
        .collect()
}

fn build_html(rows: &[SnapshotRow]) -> (String, usize, usize) {
    let created_at = {
        let d = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = d.as_secs();
        // Simple UTC timestamp (no chrono dependency)
        let (s, m, h) = (secs % 60, (secs / 60) % 60, (secs / 3600) % 24);
        let days = secs / 86400;
        // Days since epoch -> year/month/day (good enough for display)
        let (y, mo, d) = epoch_days_to_ymd(days);
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
    };
    let diff_rows: Vec<&SnapshotRow> = rows.iter().filter(|r| r.has_difference).collect();
    let new_rows: Vec<&SnapshotRow> = rows
        .iter()
        .filter(|r| r.has_current && !r.has_reference && !r.has_difference)
        .collect();

    let mut body_rows = String::new();

    for row in &diff_rows {
        body_rows.push_str(&format!(
            r#"        <tr>
          <td class="name">{name}</td>
          <td>{reference}</td>
          <td>{current}</td>
          <td>{difference}</td>
        </tr>
"#,
            name = html_escape(&row.name),
            reference = image_cell("reference", &row.name, row.has_reference),
            current = image_cell("current", &row.name, row.has_current),
            difference = image_cell("difference", &row.name, row.has_difference),
        ));
    }

    for row in &new_rows {
        body_rows.push_str(&format!(
            r#"        <tr>
          <td class="name">{name} <span class="badge new">NEW</span></td>
          <td>{reference}</td>
          <td>{current}</td>
          <td class="missing">—</td>
        </tr>
"#,
            name = html_escape(&row.name),
            reference = image_cell("reference", &row.name, row.has_reference),
            current = image_cell("current", &row.name, row.has_current),
        ));
    }

    let diff_count = diff_rows.len();
    let new_count = new_rows.len();
    let summary = format!("{diff_count} with diff, {new_count} new");

    let html = format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <title>snapvrt review</title>
  <style>
    :root {{ color-scheme: light; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      margin: 0; padding: 24px;
      background: #f6f7f9; color: #1f2933;
    }}
    h1 {{ margin: 0 0 8px; font-size: 22px; }}
    .meta {{ margin-bottom: 16px; color: #52606d; font-size: 14px; }}
    table {{ width: 100%; border-collapse: collapse; background: #fff; box-shadow: 0 2px 6px rgba(0,0,0,0.05); }}
    th, td {{ border: 1px solid #e4e7eb; padding: 8px; vertical-align: top; text-align: left; }}
    th {{ background: #f0f4f8; font-weight: 600; font-size: 14px; }}
    td img {{ max-width: 100%; height: auto; display: block; background: #fff; }}
    td {{ width: 25%; }}
    td.name {{ font-size: 13px; word-break: break-word; width: 25%; }}
    .missing {{ color: #c81e1e; font-style: italic; font-size: 13px; }}
    .badge {{ font-size: 11px; padding: 1px 6px; border-radius: 3px; font-weight: 600; }}
    .badge.new {{ background: #fef3c7; color: #92400e; }}
    .empty {{ text-align: center; padding: 48px; color: #52606d; font-size: 16px; }}
  </style>
</head>
<body>
  <h1>snapvrt review</h1>
  <div class="meta">Generated at {created_at} &middot; {summary}</div>
  {content}
</body>
</html>"##,
        created_at = created_at,
        summary = summary,
        content = if body_rows.is_empty() {
            r#"<div class="empty">All snapshots pass — nothing to review.</div>"#.to_string()
        } else {
            format!(
                r#"<table>
    <thead>
      <tr>
        <th>Name</th>
        <th>Reference</th>
        <th>Current</th>
        <th>Difference</th>
      </tr>
    </thead>
    <tbody>
{body_rows}    </tbody>
  </table>"#,
                body_rows = body_rows
            )
        }
    );

    (html, diff_count, new_count)
}

fn image_cell(subdir: &str, filename: &str, exists: bool) -> String {
    if !exists {
        return format!(r#"<div class="missing">no {subdir}</div>"#);
    }
    let safe = url_encode(filename);
    let escaped = html_escape(filename);
    format!(r#"<img src="{subdir}/{safe}" alt="{subdir} {escaped}" loading="lazy" />"#)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                String::from(b as char)
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

/// Convert days since Unix epoch to (year, month, day).
fn epoch_days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Civil calendar algorithm (Howard Hinnant)
    days += 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Generate `.snapvrt/report.html` and return the path.
pub fn generate() -> Result<String> {
    let rows = collect_rows();
    let (html, diff_count, new_count) = build_html(&rows);

    let out_path = Path::new(store::BASE_DIR).join(OUTPUT_FILE);
    std::fs::write(&out_path, html)
        .with_context(|| format!("Failed to write {}", out_path.display()))?;

    Ok(format!(
        "{} ({diff_count} with diff, {new_count} new)",
        out_path.display(),
    ))
}
