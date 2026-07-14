//! Frecency + bookmark persistence.
//!
//! A single JSON file records, per absolute path, how often it was accepted,
//! when it was last accepted, and whether it is bookmarked. The frecency score
//! (frequency weighted by recency) drives the ordering in jump mode.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Record {
    pub count: u64,
    pub last_access: u64,
    #[serde(default)]
    pub bookmarked: bool,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Store {
    pub entries: HashMap<String, Record>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `~/Library/Application Support/fzd/store.json` (macOS) or the XDG data dir
/// equivalent on other platforms.
fn store_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("fzd").join("store.json"))
}

impl Store {
    pub fn load() -> Store {
        let Some(path) = store_path() else {
            return Store::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Store::default(),
        }
    }

    /// Atomic save: write to a temp file, then rename over the target.
    pub fn save(&self) -> anyhow::Result<()> {
        let Some(path) = store_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(serde_json::to_string_pretty(self)?.as_bytes())?;
            f.flush()?;
        }
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    fn key(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    /// Record that `path` was accepted: bump frequency and recency.
    pub fn record_access(&mut self, path: &Path) {
        let rec = self.entries.entry(Self::key(path)).or_default();
        rec.count += 1;
        rec.last_access = now_secs();
    }

    /// Flip the bookmark flag; ensures the record exists.
    pub fn toggle_bookmark(&mut self, path: &Path) -> bool {
        let rec = self.entries.entry(Self::key(path)).or_default();
        rec.bookmarked = !rec.bookmarked;
        rec.bookmarked
    }

    pub fn is_bookmarked(&self, path: &Path) -> bool {
        self.entries
            .get(&Self::key(path))
            .map(|r| r.bookmarked)
            .unwrap_or(false)
    }

    /// Classic frecency: frequency scaled by how recently it was last used.
    fn score(rec: &Record, now: u64) -> f64 {
        let age = now.saturating_sub(rec.last_access);
        let mult = if age < 3600 {
            4.0
        } else if age < 86_400 {
            2.0
        } else if age < 604_800 {
            0.5
        } else {
            0.25
        };
        rec.count as f64 * mult
    }

    /// All known directories, sorted for jump mode: bookmarks first, then by
    /// descending frecency. Skips paths that no longer exist on disk.
    pub fn ranked(&self) -> Vec<RankedDir> {
        let now = now_secs();
        let mut out: Vec<RankedDir> = self
            .entries
            .iter()
            .filter(|(p, _)| Path::new(p).is_dir())
            .map(|(p, rec)| RankedDir {
                path: PathBuf::from(p),
                score: Self::score(rec, now),
                bookmarked: rec.bookmarked,
            })
            .collect();
        out.sort_by(|a, b| {
            b.bookmarked
                .cmp(&a.bookmarked)
                .then(b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
        });
        out
    }
}

pub struct RankedDir {
    pub path: PathBuf,
    pub score: f64,
    pub bookmarked: bool,
}
