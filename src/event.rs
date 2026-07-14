//! Keymap: translate key presses into [`App`] transitions.
//!
//! Because typing filters the list (fzf-style), all printable characters feed
//! the query. Commands are therefore bound to Ctrl-chords and the arrow keys.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Mode};

pub fn handle(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    // Global: Ctrl-C always cancels with no selection.
    if ctrl && matches!(key.code, KeyCode::Char('c')) {
        app.selected = None;
        app.should_quit = true;
        return;
    }

    match app.mode {
        Mode::Browse => handle_browse(app, key, ctrl),
        Mode::Jump => handle_jump(app, key, ctrl),
        Mode::Find => handle_find(app, key, ctrl),
    }
}

fn handle_browse(app: &mut App, key: KeyEvent, ctrl: bool) {
    match key.code {
        KeyCode::Esc => {
            app.selected = None;
            app.should_quit = true;
        }
        KeyCode::Enter => app.accept(),
        KeyCode::Up => app.move_cursor(-1),
        KeyCode::Down => app.move_cursor(1),
        KeyCode::Char('p') if ctrl => app.move_cursor(-1),
        KeyCode::Char('n') if ctrl => app.move_cursor(1),
        KeyCode::PageUp => app.move_cursor(-10),
        KeyCode::PageDown => app.move_cursor(10),
        KeyCode::Right | KeyCode::Tab => app.descend(),
        KeyCode::Left => app.ascend(),
        KeyCode::Backspace => {
            if app.query.is_empty() {
                app.ascend();
            } else {
                app.query.pop();
                app.cursor = 0;
                app.apply_filter();
            }
        }
        KeyCode::Char('a') if ctrl => app.toggle_hidden(),
        KeyCode::Char('b') if ctrl => app.bookmark_current(),
        KeyCode::Char('f') if ctrl => app.enter_jump_mode(),
        KeyCode::Char('s') if ctrl => app.enter_find_mode(),
        KeyCode::Char('u') if ctrl => {
            app.query.clear();
            app.cursor = 0;
            app.apply_filter();
        }
        KeyCode::Char(c) if !ctrl => {
            app.query.push(c);
            app.cursor = 0;
            app.apply_filter();
        }
        _ => {}
    }
}

fn handle_jump(app: &mut App, key: KeyEvent, ctrl: bool) {
    match key.code {
        KeyCode::Esc => app.exit_jump_mode(),
        KeyCode::Char('f') if ctrl => app.exit_jump_mode(),
        KeyCode::Enter => app.jump_accept(),
        KeyCode::Up => app.jump_move(-1),
        KeyCode::Down => app.jump_move(1),
        KeyCode::Char('p') if ctrl => app.jump_move(-1),
        KeyCode::Char('n') if ctrl => app.jump_move(1),
        KeyCode::PageUp => app.jump_move(-10),
        KeyCode::PageDown => app.jump_move(10),
        KeyCode::Backspace => {
            app.jump_query.pop();
            app.jump_cursor = 0;
            app.jump_apply_filter();
        }
        KeyCode::Char('u') if ctrl => {
            app.jump_query.clear();
            app.jump_cursor = 0;
            app.jump_apply_filter();
        }
        KeyCode::Char(c) if !ctrl => {
            app.jump_query.push(c);
            app.jump_cursor = 0;
            app.jump_apply_filter();
        }
        _ => {}
    }
}

fn handle_find(app: &mut App, key: KeyEvent, ctrl: bool) {
    match key.code {
        KeyCode::Esc => app.exit_find_mode(),
        KeyCode::Char('s') if ctrl => app.exit_find_mode(),
        KeyCode::Enter => app.find_accept(),
        KeyCode::Up => app.find_move(-1),
        KeyCode::Down => app.find_move(1),
        KeyCode::Char('p') if ctrl => app.find_move(-1),
        KeyCode::Char('n') if ctrl => app.find_move(1),
        KeyCode::PageUp => app.find_move(-10),
        KeyCode::PageDown => app.find_move(10),
        KeyCode::Backspace => {
            app.find_query.pop();
            app.find_cursor = 0;
            app.find_apply_filter();
        }
        KeyCode::Char('u') if ctrl => {
            app.find_query.clear();
            app.find_cursor = 0;
            app.find_apply_filter();
        }
        KeyCode::Char(c) if !ctrl => {
            app.find_query.push(c);
            app.find_cursor = 0;
            app.find_apply_filter();
        }
        _ => {}
    }
}
