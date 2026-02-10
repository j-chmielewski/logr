use std::time::Duration;

use crossterm::event::{read, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::{build_pattern, build_regex, max_start, AppState, LogrError};

pub(crate) struct EventResult {
    pub exit: bool,
    pub(crate) redraw: bool,
}

pub(crate) fn handle_event(
    app: &mut AppState,
    total_lines: usize,
    view_height: usize,
) -> Result<EventResult, LogrError> {
    let mut redraw = false;
    while crossterm::event::poll(Duration::from_millis(0)).unwrap_or(false) {
        if let Ok(Event::Key(KeyEvent {
            code, modifiers, ..
        })) = read()
        {
            redraw = true;
            if app.dialog_open {
                if let Some(result) = handle_dialog_event(app, code, modifiers, redraw)? {
                    return Ok(result);
                }
                continue;
            }

            if let Some(result) =
                handle_main_event(app, total_lines, view_height, code, modifiers, redraw)
            {
                return Ok(result);
            }
        }
    }

    Ok(EventResult {
        exit: false,
        redraw,
    })
}

fn handle_dialog_event(
    app: &mut AppState,
    code: KeyCode,
    modifiers: KeyModifiers,
    redraw: bool,
) -> Result<Option<EventResult>, LogrError> {
    match code {
        KeyCode::Esc => {
            app.dialog_open = false;
            app.input.clear();
            app.pattern_error = None;
        }
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
            return Ok(Some(EventResult { exit: true, redraw }));
        }
        KeyCode::Enter => {
            if !app.input.trim().is_empty() {
                match build_pattern(app.input.clone(), !app.ignore_case) {
                    Ok(pattern) => {
                        app.patterns.push(pattern);
                        app.dialog_open = false;
                        app.input.clear();
                        app.pattern_error = None;
                    }
                    Err(err) => {
                        app.pattern_error = Some(format!("Invalid pattern: {err}"));
                    }
                }
            } else {
                app.dialog_open = false;
                app.pattern_error = None;
            }
        }
        KeyCode::Up => {
            if app.selected > 0 {
                app.selected -= 1;
            }
        }
        KeyCode::Down => {
            if app.selected < app.patterns.len() {
                app.selected += 1;
            }
        }
        KeyCode::Left | KeyCode::Right => {
            if app.selected < app.patterns.len() {
                let case_sensitive = !app.patterns[app.selected].case_sensitive;
                match build_regex(&app.patterns[app.selected].pattern, case_sensitive) {
                    Ok(regex) => {
                        app.patterns[app.selected].case_sensitive = case_sensitive;
                        app.patterns[app.selected].regex = regex;
                    }
                    Err(err) => {
                        app.pattern_error = Some(format!("Invalid pattern: {err}"));
                    }
                }
            }
        }
        KeyCode::Delete => {
            if app.selected < app.patterns.len() {
                app.patterns.remove(app.selected);
                if app.selected > app.patterns.len() {
                    app.selected = app.patterns.len();
                }
                if app.patterns.is_empty() {
                    app.selected = 0;
                }
            }
        }
        KeyCode::Backspace => {
            app.input.pop();
            app.selected = app.patterns.len();
        }
        KeyCode::Char(c) => {
            if !modifiers.contains(KeyModifiers::CONTROL) {
                app.input.push(c);
                app.selected = app.patterns.len();
            }
        }
        _ => {}
    }

    Ok(None)
}

fn handle_main_event(
    app: &mut AppState,
    total_lines: usize,
    view_height: usize,
    code: KeyCode,
    modifiers: KeyModifiers,
    redraw: bool,
) -> Option<EventResult> {
    match code {
        KeyCode::Char('q') => return Some(EventResult { exit: true, redraw }),
        KeyCode::Char('p') => {
            app.dialog_open = true;
            app.input.clear();
            app.pattern_error = None;
            app.selected = 0;
        }
        KeyCode::Char('w') => {
            app.wrap = !app.wrap;
        }
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
            return Some(EventResult { exit: true, redraw });
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if total_lines > 0 {
                let max_start = max_start(total_lines, view_height);
                if app.follow {
                    app.follow = false;
                    app.scroll = max_start;
                }
                if app.scroll > 0 {
                    app.scroll -= 1;
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if total_lines > 0 {
                let max_start = max_start(total_lines, view_height);
                if app.follow {
                    app.scroll = max_start;
                }
                if app.scroll < max_start {
                    app.scroll += 1;
                } else {
                    app.follow = true;
                }
            }
        }
        KeyCode::PageUp | KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            if total_lines > 0 {
                let max_start = max_start(total_lines, view_height);
                let delta = usize::max(1, view_height / 2);
                if app.follow {
                    app.follow = false;
                    app.scroll = max_start;
                }
                app.scroll = app.scroll.saturating_sub(delta);
            }
        }
        KeyCode::PageDown | KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            if total_lines > 0 {
                let max_start = max_start(total_lines, view_height);
                let delta = usize::max(1, view_height / 2);
                if app.follow {
                    app.scroll = max_start;
                }
                app.scroll = usize::min(app.scroll + delta, max_start);
                if app.scroll == max_start {
                    app.follow = true;
                }
            }
        }
        KeyCode::Home | KeyCode::Char('g') if !modifiers.contains(KeyModifiers::SHIFT) => {
            app.follow = false;
            app.scroll = 0;
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.follow = true;
            app.scroll = max_start(total_lines, view_height);
        }
        _ => {}
    }

    None
}
