//! Ratatui rendering for both modes.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{App, Mode};
use crate::fs;

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // header: border + 2 content lines + border
            Constraint::Min(1),    // body
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    match app.mode {
        Mode::Browse => {
            render_header(f, chunks[0], app);
            render_browse_body(f, chunks[1], app);
        }
        Mode::Jump => {
            render_jump_header(f, chunks[0], app);
            render_jump_body(f, chunks[1], app);
        }
        Mode::Find => {
            render_find_header(f, chunks[0], app);
            render_find_body(f, chunks[1], app);
        }
    }
    render_footer(f, chunks[2], app);
}

/// A live search line: `🔍 <typed text>▌`, with a visible cursor so the user
/// can always see what they type. Falls back to a dim placeholder when empty.
fn query_line(query: &str, placeholder: &str, color: Color) -> Line<'static> {
    let mut spans = vec![Span::raw("🔍 ")];
    spans.push(Span::styled(query.to_string(), Style::default().fg(color).bold()));
    spans.push(Span::styled("▌", Style::default().fg(color))); // cursor caret
    if query.is_empty() {
        spans.push(Span::styled(
            format!("  {placeholder}"),
            Style::default().fg(DIM),
        ));
    }
    Line::from(spans)
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let path = app.cwd.display().to_string();
    let lines = vec![
        Line::from(vec![
            Span::styled("📂 ", Style::default().fg(ACCENT)),
            Span::styled(path, Style::default().fg(ACCENT).bold()),
        ]),
        query_line(&app.query, "type to filter", ACCENT),
    ];
    let block = Block::default().borders(Borders::ALL).title(" fzd ");
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_browse_body(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    // File list
    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&i| {
            let e = &app.entries[i];
            let (icon, mut style) = if e.is_dir {
                ("▸ ", Style::default().fg(ACCENT))
            } else {
                ("  ", Style::default())
            };
            if e.is_hidden {
                style = style.fg(DIM);
            }
            let mut name = e.name.clone();
            if e.is_dir {
                name.push('/');
            }
            let mut spans = vec![Span::styled(icon, style), Span::styled(name, style)];
            if app.store.is_bookmarked(&e.path) {
                spans.push(Span::styled("  ★", Style::default().fg(Color::Yellow)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let count = format!(" {} items ", app.filtered.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(count))
        .highlight_style(Style::default().bg(ACCENT).fg(Color::Black).bold())
        .highlight_symbol("");

    let mut state = ListState::default();
    if !app.filtered.is_empty() {
        state.select(Some(app.cursor));
    }
    f.render_stateful_widget(list, cols[0], &mut state);

    // Info panel
    render_info_panel(f, cols[1], app);
}

fn render_info_panel(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title(" info ");
    let lines = match app.current_entry() {
        None => vec![Line::from(Span::styled("empty", Style::default().fg(DIM)))],
        Some(e) => {
            let mut lines = vec![
                Line::from(Span::styled(&e.name, Style::default().bold())),
                Line::from(Span::styled(
                    if e.is_dir { "directory" } else { "file" },
                    Style::default().fg(DIM),
                )),
                Line::raw(""),
            ];
            if e.is_dir {
                let n = fs::dir_child_count(&e.path)
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "?".into());
                lines.push(kv("items", &n));
            } else {
                lines.push(kv("size", &fs::human_size(e.size)));
            }
            if let Some(m) = e.modified {
                lines.push(kv("modified", &fs::human_time_ago(m)));
            }
            if app.store.is_bookmarked(&e.path) {
                lines.push(Line::from(Span::styled(
                    "★ bookmarked",
                    Style::default().fg(Color::Yellow),
                )));
            }
            lines
        }
    };
    f.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn kv(key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key}: "), Style::default().fg(DIM)),
        Span::raw(value.to_string()),
    ])
}

fn render_jump_header(f: &mut Frame, area: Rect, app: &App) {
    let lines = vec![
        Line::from(Span::styled("⚡ jump", Style::default().fg(Color::Magenta).bold())),
        query_line(
            &app.jump_query,
            "fuzzy-search frecent + bookmarked dirs",
            Color::Magenta,
        ),
    ];
    let block = Block::default().borders(Borders::ALL).title(" fzd · jump ");
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_jump_body(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .jump_filtered
        .iter()
        .map(|&i| {
            let e = &app.jump_entries[i];
            let mut spans = Vec::new();
            if e.bookmarked {
                spans.push(Span::styled("★ ", Style::default().fg(Color::Yellow)));
            } else {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::raw(e.display.clone()));
            ListItem::new(Line::from(spans))
        })
        .collect();

    let count = format!(" {} dirs ", app.jump_filtered.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(count))
        .highlight_style(Style::default().bg(Color::Magenta).fg(Color::Black).bold());

    let mut state = ListState::default();
    if !app.jump_filtered.is_empty() {
        state.select(Some(app.jump_cursor));
    }
    f.render_stateful_widget(list, area, &mut state);
}

const FIND_COLOR: Color = Color::Green;

fn render_find_header(f: &mut Frame, area: Rect, app: &App) {
    let lines = vec![
        Line::from(vec![
            Span::styled("🔎 find under ", Style::default().fg(FIND_COLOR).bold()),
            Span::styled(app.find_root.display().to_string(), Style::default().fg(DIM)),
        ]),
        query_line(
            &app.find_query,
            "recursively fuzzy-find dirs below here",
            FIND_COLOR,
        ),
    ];
    let block = Block::default().borders(Borders::ALL).title(" fzd · find ");
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_find_body(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .find_filtered
        .iter()
        .map(|&i| {
            let e = &app.find_entries[i];
            let bookmarked = app.store.is_bookmarked(&e.path);
            let mut spans = vec![Span::styled(
                "▸ ",
                Style::default().fg(FIND_COLOR),
            )];
            spans.push(Span::raw(e.display.clone()));
            if bookmarked {
                spans.push(Span::styled("  ★", Style::default().fg(Color::Yellow)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let count = format!(" {} matches ", app.find_filtered.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(count))
        .highlight_style(Style::default().bg(FIND_COLOR).fg(Color::Black).bold());

    let mut state = ListState::default();
    if !app.find_filtered.is_empty() {
        state.select(Some(app.find_cursor));
    }
    f.render_stateful_widget(list, area, &mut state);
}

fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let help = match app.mode {
        Mode::Browse => {
            "↑↓ move  ⏎ cd here  → open  ← up  ^S find  ^F jump  ^A hidden  ^B bookmark  esc quit"
        }
        Mode::Jump => "↑↓ move  ⏎ cd  esc back  ^F browse",
        Mode::Find => "↑↓ move  ⏎ cd  esc back  ^S browse",
    };
    let text = match &app.status {
        Some(s) => Line::from(Span::styled(s.clone(), Style::default().fg(Color::Yellow))),
        None => Line::from(Span::styled(help, Style::default().fg(DIM))),
    };
    f.render_widget(Paragraph::new(text), area);
}
