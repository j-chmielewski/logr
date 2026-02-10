use clap::{ArgAction, Parser};
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers, read,
    },
    execute,
    terminal::{
        Clear as TermClear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
        disable_raw_mode, enable_raw_mode,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};
use regex::{Regex, RegexBuilder};
use std::{
    io::{self, Stdout},
    time::Duration,
};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    time::timeout,
};

use crate::ui::ui;

mod ui;

const TICK_RATE: Duration = Duration::from_millis(20);

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, num_args = 0.., value_delimiter = ',')]
    patterns: Vec<String>,

    #[arg(short, long, action = ArgAction::SetTrue)]
    ignore_case: bool,
}

#[derive(Error, Debug)]
pub enum LogrError {
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    RegexError(#[from] regex::Error),
}

pub(crate) struct PatternSpec {
    pattern: String,
    case_sensitive: bool,
    regex: Regex,
}

struct AppState {
    patterns: Vec<PatternSpec>,
    selected: usize,
    dialog_open: bool,
    input: String,
    pattern_error: Option<String>,
    ignore_case: bool,
    scroll: usize,
    follow: bool,
    wrap: bool,
}

impl AppState {
    #[must_use]
    pub fn new(patterns: Vec<PatternSpec>, ignore_case: bool) -> Self {
        Self {
            patterns,
            selected: 0,
            dialog_open: false,
            input: String::new(),
            pattern_error: None,
            ignore_case,
            scroll: 0,
            follow: true,
            wrap: false,
        }
    }
}

pub async fn run(args: Args) -> Result<(), LogrError> {
    let mut patterns = Vec::new();
    for pattern in &args.patterns {
        patterns.push(build_pattern(pattern.clone(), !args.ignore_case)?);
    }
    let mut app = AppState::new(patterns, args.ignore_case);

    let mut terminal = term_init()?;
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines_stream = stdin.lines();
    let mut lines = Vec::new();

    loop {
        let view_height = terminal.size()?.height.saturating_sub(2) as usize;
        let event_result = handle_event(&mut app, lines.len(), view_height)?;
        if event_result.exit {
            break;
        }

        let mut should_draw = event_result.redraw || app.dialog_open;
        if let Ok(Ok(Some(line))) = timeout(TICK_RATE, lines_stream.next_line()).await {
            lines.push(line);
            should_draw = true;
        }

        if should_draw {
            terminal.draw(|f| ui(f, &lines, &app))?;
        }
    }

    term_cleanup(terminal)?;

    Ok(())
}

type LogrTerminal = Terminal<CrosstermBackend<Stdout>>;

fn term_init() -> Result<LogrTerminal, io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        TermClear(ClearType::All),
        EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn term_cleanup(mut terminal: LogrTerminal) -> Result<(), io::Error> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()
}

fn build_regex(pattern: &str, case_sensitive: bool) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .build()
}

fn build_pattern(pattern: String, case_sensitive: bool) -> Result<PatternSpec, LogrError> {
    let regex = build_regex(&pattern, case_sensitive)?;
    Ok(PatternSpec {
        pattern,
        case_sensitive,
        regex,
    })
}

struct EventResult {
    exit: bool,
    redraw: bool,
}

fn handle_event(
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
                match code {
                    KeyCode::Esc => {
                        app.dialog_open = false;
                        app.input.clear();
                        app.pattern_error = None;
                    }
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(EventResult { exit: true, redraw });
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
                            let case_sensitive =
                                !app.patterns[app.selected].case_sensitive;
                            match build_regex(
                                &app.patterns[app.selected].pattern,
                                case_sensitive,
                            ) {
                                Ok(regex) => {
                                    app.patterns[app.selected].case_sensitive = case_sensitive;
                                    app.patterns[app.selected].regex = regex;
                                }
                                Err(err) => {
                                    app.pattern_error =
                                        Some(format!("Invalid pattern: {err}"));
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
                continue;
            }

            match code {
                KeyCode::Char('q') => return Ok(EventResult { exit: true, redraw }),
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
                    return Ok(EventResult { exit: true, redraw });
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
                KeyCode::PageUp | KeyCode::Char('u')
                    if modifiers.contains(KeyModifiers::CONTROL) =>
                {
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
                KeyCode::PageDown | KeyCode::Char('d')
                    if modifiers.contains(KeyModifiers::CONTROL) =>
                {
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
        }
    }

    Ok(EventResult {
        exit: false,
        redraw,
    })
}

fn max_start(total_lines: usize, view_height: usize) -> usize {
    if view_height == 0 {
        0
    } else {
        total_lines.saturating_sub(view_height)
    }
}
