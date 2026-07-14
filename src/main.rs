//! fzd — an interactive terminal directory explorer that cd's your shell.
//!
//! The TUI is drawn to **stderr** so that stdout stays clean; on accept the
//! chosen absolute path is the only thing printed to stdout. A small fish
//! wrapper (`shell/fzd.fish`) captures that path and runs the real `cd`.

mod app;
mod event;
mod fs;
mod fuzzy;
mod store;
mod ui;
mod update;

use std::io::{self, Stderr};
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self as cevent, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;
use store::Store;

#[derive(Parser)]
#[command(name = "fzd", version, about = "Interactive terminal directory explorer")]
struct Args {
    /// Directory to start in (defaults to the current directory).
    path: Option<PathBuf>,

    /// Start directly in jump mode (frecent + bookmarked dirs).
    #[arg(short, long)]
    jump: bool,

    /// Show hidden (dot) files from the start.
    #[arg(short, long)]
    all: bool,

    /// Do not record the accepted directory in the frecency store.
    #[arg(long)]
    print_only: bool,

    /// Check GitHub for a newer release, then exit.
    #[arg(long)]
    check_update: bool,

    /// Download the latest GitHub release and replace this binary, then exit.
    #[arg(long)]
    update: bool,

    /// With --update, self-replace even a package-manager-managed binary.
    #[arg(long)]
    force: bool,
}

/// RAII terminal guard so we always restore the terminal, even on error/panic.
struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stderr>>,
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut out = io::stderr();
        execute!(out, EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(io::stderr()))?;
        Ok(TerminalGuard { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stderr(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.check_update {
        return update::check_and_report();
    }
    if args.update {
        return update::update(args.force);
    }

    let start = match args.path {
        Some(p) => p.canonicalize().unwrap_or(p),
        None => std::env::current_dir()?,
    };

    let store = Store::load();
    let mut app = App::new(start, store, args.all);
    if args.jump {
        app.enter_jump_mode();
    }

    run(&mut app)?;

    // Terminal is restored here (guard dropped inside run). Emit the result.
    if let Some(sel) = app.selected.take() {
        if !args.print_only {
            app.store.record_access(&sel);
            let _ = app.store.save();
        }
        println!("{}", sel.display());
    }
    Ok(())
}

fn run(app: &mut App) -> Result<()> {
    let mut guard = TerminalGuard::new()?;
    loop {
        guard.terminal.draw(|f| ui::render(f, app))?;
        if let Event::Key(key) = cevent::read()? {
            // Ignore key-release / repeat noise on platforms that emit it.
            if key.kind == KeyEventKind::Press {
                event::handle(app, key);
            }
        }
        if app.should_quit {
            break;
        }
    }
    Ok(())
}
