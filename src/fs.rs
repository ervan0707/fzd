//! Reading a single directory level and pulling lightweight metadata.
//!
//! We keep the per-entry read cheap (only what `DirEntry::metadata` already
//! gives us). Anything expensive — like counting the children of a subdirectory
//! — is computed lazily via [`dir_child_count`] for the highlighted entry only,
//! so opening a directory with thousands of entries stays snappy.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_hidden: bool,
    /// File size in bytes (0 for directories).
    pub size: u64,
    pub modified: Option<SystemTime>,
}

/// Read one directory level. Directories sort first, then case-insensitive name.
pub fn read_dir(path: &Path, show_hidden: bool) -> anyhow::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for de in fs::read_dir(path)? {
        let de = match de {
            Ok(d) => d,
            Err(_) => continue,
        };
        let name = de.file_name().to_string_lossy().into_owned();
        let is_hidden = name.starts_with('.');
        if is_hidden && !show_hidden {
            continue;
        }
        let meta = de.metadata().ok();
        // Resolve symlinks-to-dirs as directories too.
        let is_dir = match &meta {
            Some(m) if m.is_symlink() => de.path().is_dir(),
            Some(m) => m.is_dir(),
            None => de.path().is_dir(),
        };
        let size = if is_dir {
            0
        } else {
            meta.as_ref().map(|m| m.len()).unwrap_or(0)
        };
        let modified = meta.as_ref().and_then(|m| m.modified().ok());
        entries.push(Entry {
            name,
            path: de.path(),
            is_dir,
            is_hidden,
            size,
            modified,
        });
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

/// Count immediate children of a directory (lazy; used for the info panel).
pub fn dir_child_count(path: &Path) -> Option<usize> {
    fs::read_dir(path).ok().map(|it| it.count())
}

pub struct FoundDir {
    pub path: PathBuf,
    /// Path relative to the walk root, for display + fuzzy matching.
    pub rel: String,
}

/// Directory names never descended into (noise + huge trees).
const SKIP_DIRS: [&str; 8] = [
    ".git",
    "node_modules",
    "target",
    ".direnv",
    ".cache",
    "result",
    ".venv",
    "__pycache__",
];

/// Recursively collect directories under `root` (depth-first), for the
/// recursive find mode. Skips [`SKIP_DIRS`], honours `show_hidden`, does not
/// follow directory symlinks (cycle safety), and stops at `max_depth` / `limit`.
/// Returns `(dirs, truncated)` where `truncated` is true if `limit` was hit.
pub fn walk_dirs(
    root: &Path,
    show_hidden: bool,
    max_depth: usize,
    limit: usize,
) -> (Vec<FoundDir>, bool) {
    let mut out: Vec<FoundDir> = Vec::new();
    let mut truncated = false;
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];

    while let Some((dir, depth)) = stack.pop() {
        if depth >= max_depth {
            continue;
        }
        let rd = match fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for de in rd.flatten() {
            let name = de.file_name().to_string_lossy().into_owned();
            let is_hidden = name.starts_with('.');
            if (is_hidden && !show_hidden) || SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            let ft = de.file_type().ok();
            let is_symlink = ft.as_ref().map(|f| f.is_symlink()).unwrap_or(false);
            let path = de.path();
            let is_dir = if is_symlink {
                path.is_dir()
            } else {
                ft.as_ref().map(|f| f.is_dir()).unwrap_or(false)
            };
            if !is_dir {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            out.push(FoundDir {
                path: path.clone(),
                rel,
            });
            if out.len() >= limit {
                truncated = true;
                break;
            }
            // Don't descend through symlinks — avoids cycles.
            if !is_symlink {
                stack.push((path, depth + 1));
            }
        }
        if truncated {
            break;
        }
    }
    out.sort_by(|a, b| a.rel.to_lowercase().cmp(&b.rel.to_lowercase()));
    (out, truncated)
}

/// Human-readable byte size.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{size:.1}{}", UNITS[unit])
    }
}

/// Rough "time ago" for a modified timestamp.
pub fn human_time_ago(t: SystemTime) -> String {
    let secs = t
        .elapsed()
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 2_592_000 {
        format!("{}d ago", secs / 86400)
    } else {
        format!("{}mo ago", secs / 2_592_000)
    }
}
