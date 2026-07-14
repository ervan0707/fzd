//! Application state and the transitions the event layer drives.

use std::path::PathBuf;

use fuzzy_matcher::skim::SkimMatcherV2;

use crate::fs::{self, Entry};
use crate::fuzzy;
use crate::store::Store;

#[derive(PartialEq, Eq)]
pub enum Mode {
    Browse,
    Jump,
    Find,
}

pub struct JumpEntry {
    pub path: PathBuf,
    pub display: String,
    pub bookmarked: bool,
}

pub struct FindEntry {
    pub path: PathBuf,
    /// Path relative to the find root.
    pub display: String,
}

pub struct App {
    pub mode: Mode,
    pub cwd: PathBuf,

    // Browse mode
    pub entries: Vec<Entry>,
    pub filtered: Vec<usize>,
    pub cursor: usize,
    pub query: String,
    pub show_hidden: bool,

    // Jump mode
    pub jump_entries: Vec<JumpEntry>,
    pub jump_filtered: Vec<usize>,
    pub jump_cursor: usize,
    pub jump_query: String,

    // Recursive find mode
    pub find_root: PathBuf,
    pub find_entries: Vec<FindEntry>,
    pub find_filtered: Vec<usize>,
    pub find_cursor: usize,
    pub find_query: String,

    pub store: Store,
    pub matcher: SkimMatcherV2,
    pub status: Option<String>,

    /// Set when the user accepts a directory; the caller cd's here.
    pub selected: Option<PathBuf>,
    pub should_quit: bool,
}

impl App {
    pub fn new(start: PathBuf, store: Store, show_hidden: bool) -> App {
        let mut app = App {
            mode: Mode::Browse,
            cwd: start,
            entries: Vec::new(),
            filtered: Vec::new(),
            cursor: 0,
            query: String::new(),
            show_hidden,
            jump_entries: Vec::new(),
            jump_filtered: Vec::new(),
            jump_cursor: 0,
            jump_query: String::new(),
            find_root: PathBuf::new(),
            find_entries: Vec::new(),
            find_filtered: Vec::new(),
            find_cursor: 0,
            find_query: String::new(),
            store,
            matcher: SkimMatcherV2::default(),
            status: None,
            selected: None,
            should_quit: false,
        };
        app.reload();
        app
    }

    // ---- Browse mode ----

    /// Re-read `cwd` and reapply the current filter. `keep` lets the caller
    /// re-select a folder by name (used when ascending back to a parent).
    pub fn reload(&mut self) {
        match fs::read_dir(&self.cwd, self.show_hidden) {
            Ok(entries) => {
                self.entries = entries;
                self.status = None;
            }
            Err(e) => {
                self.entries = Vec::new();
                self.status = Some(format!("cannot read directory: {e}"));
            }
        }
        self.apply_filter();
    }

    pub fn apply_filter(&mut self) {
        let names: Vec<String> = self.entries.iter().map(|e| e.name.clone()).collect();
        self.filtered = fuzzy::filter(&self.matcher, &names, &self.query);
        if self.cursor >= self.filtered.len() {
            self.cursor = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn current_entry(&self) -> Option<&Entry> {
        self.filtered
            .get(self.cursor)
            .and_then(|&i| self.entries.get(i))
    }

    pub fn move_cursor(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            return;
        }
        let len = self.filtered.len() as isize;
        let mut next = self.cursor as isize + delta;
        if next < 0 {
            next = 0;
        } else if next >= len {
            next = len - 1;
        }
        self.cursor = next as usize;
    }

    /// Descend into the highlighted directory, keep browsing.
    pub fn descend(&mut self) {
        if let Some(entry) = self.current_entry() {
            if entry.is_dir {
                self.cwd = entry.path.clone();
                self.query.clear();
                self.cursor = 0;
                self.reload();
            }
        }
    }

    /// Go up one level, re-selecting the folder we came from.
    pub fn ascend(&mut self) {
        let child = self
            .cwd
            .file_name()
            .map(|n| n.to_string_lossy().into_owned());
        if let Some(parent) = self.cwd.parent().map(|p| p.to_path_buf()) {
            if parent == self.cwd {
                return; // already at root
            }
            self.cwd = parent;
            self.query.clear();
            self.cursor = 0;
            self.reload();
            if let Some(name) = child {
                if let Some(pos) = self
                    .filtered
                    .iter()
                    .position(|&i| self.entries[i].name == name)
                {
                    self.cursor = pos;
                }
            }
        }
    }

    /// Accept a directory and quit: the highlighted folder if it is one,
    /// otherwise the current directory itself.
    pub fn accept(&mut self) {
        let target = match self.current_entry() {
            Some(e) if e.is_dir => e.path.clone(),
            _ => self.cwd.clone(),
        };
        self.selected = Some(target);
        self.should_quit = true;
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.reload();
        self.status = Some(format!(
            "hidden files: {}",
            if self.show_hidden { "shown" } else { "hidden" }
        ));
    }

    /// Bookmark the highlighted directory (or cwd if the highlight is a file).
    pub fn bookmark_current(&mut self) {
        let target = match self.current_entry() {
            Some(e) if e.is_dir => e.path.clone(),
            _ => self.cwd.clone(),
        };
        let on = self.store.toggle_bookmark(&target);
        let _ = self.store.save();
        self.status = Some(format!(
            "{} {}",
            if on { "bookmarked" } else { "un-bookmarked" },
            target.display()
        ));
    }

    // ---- Jump mode ----

    pub fn enter_jump_mode(&mut self) {
        self.mode = Mode::Jump;
        self.jump_query.clear();
        self.jump_cursor = 0;
        self.jump_entries = self
            .store
            .ranked()
            .into_iter()
            .map(|r| JumpEntry {
                display: r.path.to_string_lossy().into_owned(),
                path: r.path,
                bookmarked: r.bookmarked,
            })
            .collect();
        self.jump_apply_filter();
        if self.jump_entries.is_empty() {
            self.status = Some("no frecent/bookmarked dirs yet — accept some first".into());
        }
    }

    pub fn exit_jump_mode(&mut self) {
        self.mode = Mode::Browse;
        self.status = None;
    }

    pub fn jump_apply_filter(&mut self) {
        let names: Vec<String> = self.jump_entries.iter().map(|e| e.display.clone()).collect();
        self.jump_filtered = fuzzy::filter(&self.matcher, &names, &self.jump_query);
        if self.jump_cursor >= self.jump_filtered.len() {
            self.jump_cursor = self.jump_filtered.len().saturating_sub(1);
        }
    }

    pub fn jump_move(&mut self, delta: isize) {
        if self.jump_filtered.is_empty() {
            return;
        }
        let len = self.jump_filtered.len() as isize;
        let mut next = self.jump_cursor as isize + delta;
        if next < 0 {
            next = 0;
        } else if next >= len {
            next = len - 1;
        }
        self.jump_cursor = next as usize;
    }

    pub fn jump_current(&self) -> Option<&JumpEntry> {
        self.jump_filtered
            .get(self.jump_cursor)
            .and_then(|&i| self.jump_entries.get(i))
    }

    pub fn jump_accept(&mut self) {
        if let Some(entry) = self.jump_current() {
            self.selected = Some(entry.path.clone());
            self.should_quit = true;
        }
    }

    // ---- Recursive find mode ----

    const FIND_MAX_DEPTH: usize = 12;
    const FIND_LIMIT: usize = 20_000;

    pub fn enter_find_mode(&mut self) {
        self.mode = Mode::Find;
        self.find_query.clear();
        self.find_cursor = 0;
        self.find_root = self.cwd.clone();
        let (found, truncated) = fs::walk_dirs(
            &self.cwd,
            self.show_hidden,
            Self::FIND_MAX_DEPTH,
            Self::FIND_LIMIT,
        );
        self.find_entries = found
            .into_iter()
            .map(|f| FindEntry {
                display: f.rel,
                path: f.path,
            })
            .collect();
        self.find_apply_filter();
        self.status = Some(if truncated {
            format!(
                "found {}+ dirs (capped) under {}",
                self.find_entries.len(),
                self.cwd.display()
            )
        } else {
            format!(
                "found {} dirs under {}",
                self.find_entries.len(),
                self.cwd.display()
            )
        });
    }

    pub fn exit_find_mode(&mut self) {
        self.mode = Mode::Browse;
        self.status = None;
    }

    pub fn find_apply_filter(&mut self) {
        let names: Vec<String> = self.find_entries.iter().map(|e| e.display.clone()).collect();
        self.find_filtered = fuzzy::filter(&self.matcher, &names, &self.find_query);
        if self.find_cursor >= self.find_filtered.len() {
            self.find_cursor = self.find_filtered.len().saturating_sub(1);
        }
    }

    pub fn find_move(&mut self, delta: isize) {
        if self.find_filtered.is_empty() {
            return;
        }
        let len = self.find_filtered.len() as isize;
        let mut next = self.find_cursor as isize + delta;
        if next < 0 {
            next = 0;
        } else if next >= len {
            next = len - 1;
        }
        self.find_cursor = next as usize;
    }

    pub fn find_current(&self) -> Option<&FindEntry> {
        self.find_filtered
            .get(self.find_cursor)
            .and_then(|&i| self.find_entries.get(i))
    }

    pub fn find_accept(&mut self) {
        if let Some(entry) = self.find_current() {
            self.selected = Some(entry.path.clone());
            self.should_quit = true;
        }
    }
}
