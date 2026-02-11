use std::path::Path;

use anyhow::{Context, Result};

pub const BASE_DIR: &str = ".snapvrt";
pub const REFERENCE_DIR: &str = "reference";
pub const CURRENT_DIR: &str = "current";
pub const DIFFERENCE_DIR: &str = "difference";

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    Ok(())
}

fn file_path(subdir: &str, id: &str) -> std::path::PathBuf {
    Path::new(BASE_DIR).join(subdir).join(format!("{id}.png"))
}

pub fn write_reference(id: &str, png: &[u8]) -> Result<()> {
    let path = file_path(REFERENCE_DIR, id);
    ensure_parent(&path)?;
    std::fs::write(&path, png).with_context(|| format!("Failed to write {}", path.display()))?;
    // Clean stale current/difference for this id
    let _ = std::fs::remove_file(file_path(CURRENT_DIR, id));
    let _ = std::fs::remove_file(file_path(DIFFERENCE_DIR, id));
    Ok(())
}

pub fn write_current(id: &str, png: &[u8]) -> Result<()> {
    let path = file_path(CURRENT_DIR, id);
    ensure_parent(&path)?;
    std::fs::write(&path, png).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

pub fn write_difference(id: &str, png: &[u8]) -> Result<()> {
    let path = file_path(DIFFERENCE_DIR, id);
    ensure_parent(&path)?;
    std::fs::write(&path, png).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

pub fn read_reference(id: &str) -> Option<Vec<u8>> {
    let path = file_path(REFERENCE_DIR, id);
    std::fs::read(&path).ok()
}

pub fn clean_output(id: &str) {
    let _ = std::fs::remove_file(file_path(CURRENT_DIR, id));
    let _ = std::fs::remove_file(file_path(DIFFERENCE_DIR, id));
}

/// Remove all files from `current/` and `difference/` directories.
pub fn clear_output_dirs() {
    for subdir in [CURRENT_DIR, DIFFERENCE_DIR] {
        let dir = Path::new(BASE_DIR).join(subdir);
        if dir.exists() {
            let _ = std::fs::remove_dir_all(&dir);
            let _ = std::fs::create_dir_all(&dir);
        }
    }
}

/// Remove `current/` and `difference/` files for the given snapshot IDs only.
pub fn clean_output_files(ids: &[String]) {
    for id in ids {
        let _ = std::fs::remove_file(file_path(CURRENT_DIR, id));
        let _ = std::fs::remove_file(file_path(DIFFERENCE_DIR, id));
    }
}

/// Recursively walk a directory, collecting all `.png` files as IDs
/// (relative path without the `.png` extension).
fn collect_png_ids(base: &Path, dir: &Path, ids: &mut std::collections::BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_png_ids(base, &path, ids);
        } else if path.extension().is_some_and(|e| e == "png")
            && let Ok(rel) = path.strip_prefix(base)
        {
            // Strip the .png extension to get the ID
            let id = rel.with_extension("");
            ids.insert(id.to_string_lossy().into_owned());
        }
    }
}

pub fn list_current_ids() -> std::collections::BTreeSet<String> {
    let dir = Path::new(BASE_DIR).join(CURRENT_DIR);
    let mut ids = std::collections::BTreeSet::new();
    collect_png_ids(&dir, &dir, &mut ids);
    ids
}

pub fn list_reference_ids() -> std::collections::BTreeSet<String> {
    let dir = Path::new(BASE_DIR).join(REFERENCE_DIR);
    let mut ids = std::collections::BTreeSet::new();
    collect_png_ids(&dir, &dir, &mut ids);
    ids
}

/// Delete a reference PNG and clean up empty parent directories.
pub fn remove_reference(id: &str) {
    let path = file_path(REFERENCE_DIR, id);
    let _ = std::fs::remove_file(&path);
    // Walk up and remove empty parent dirs up to the reference root.
    let root = Path::new(BASE_DIR).join(REFERENCE_DIR);
    let mut dir = path.parent();
    while let Some(d) = dir {
        if d == root {
            break;
        }
        if std::fs::read_dir(d).map_or(true, |mut e| e.next().is_none()) {
            let _ = std::fs::remove_dir(d);
            dir = d.parent();
        } else {
            break;
        }
    }
}

pub fn has_difference(id: &str) -> bool {
    file_path(DIFFERENCE_DIR, id).exists()
}

pub fn read_current(id: &str) -> Option<Vec<u8>> {
    let path = file_path(CURRENT_DIR, id);
    std::fs::read(&path).ok()
}
