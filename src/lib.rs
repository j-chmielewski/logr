use clap::{ArgAction, Parser};
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture
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

use crate::{event::handle_event, ui::ui};

mod event;
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

pub struct PatternSpec {
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
    filter_only: bool,
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
            filter_only: false,
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
        let total_lines = filtered_line_count(&lines, &app.patterns, app.filter_only);
        let view_height = terminal.size()?.height.saturating_sub(2) as usize;
        let event_result = handle_event(&mut app, total_lines, view_height)?;
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

fn max_start(total_lines: usize, view_height: usize) -> usize {
    if view_height == 0 {
        0
    } else {
        total_lines.saturating_sub(view_height)
    }
}

pub(crate) fn line_matches_patterns(line: &str, patterns: &[PatternSpec]) -> bool {
    patterns.iter().any(|pattern| pattern.regex.is_match(line))
}

fn filtered_line_count(lines: &[String], patterns: &[PatternSpec], filter_only: bool) -> usize {
    if !filter_only {
        return lines.len();
    }
    lines
        .iter()
        .filter(|line| line_matches_patterns(line, patterns))
        .count()
}

#[cfg(test)]
mod tests {
    use super::{build_pattern, build_regex, line_matches_patterns, max_start};

    #[test]
    fn build_regex_respects_case_sensitivity() {
        let sensitive = build_regex("foo", true).expect("regex build failed");
        let insensitive = build_regex("foo", false).expect("regex build failed");

        assert!(sensitive.is_match("foo"));
        assert!(!sensitive.is_match("FOO"));
        assert!(insensitive.is_match("FOO"));
    }

    #[test]
    fn build_pattern_sets_fields() {
        let pattern = build_pattern("bar".to_string(), true).expect("pattern build failed");
        assert_eq!(pattern.pattern, "bar");
        assert!(pattern.case_sensitive);
        assert!(pattern.regex.is_match("bar"));
        assert!(!pattern.regex.is_match("BAR"));
    }

    #[test]
    fn max_start_handles_empty_and_small_windows() {
        assert_eq!(max_start(0, 10), 0);
        assert_eq!(max_start(5, 10), 0);
        assert_eq!(max_start(10, 10), 0);
        assert_eq!(max_start(11, 10), 1);
        assert_eq!(max_start(100, 10), 90);
        assert_eq!(max_start(100, 0), 0);
    }

    #[test]
    fn line_match_returns_true_for_matching_pattern() {
        let patterns = vec![build_pattern("error".to_string(), true).expect("pattern build failed")];
        assert!(line_matches_patterns("error happened", &patterns));
        assert!(!line_matches_patterns("all good", &patterns));
    }
}
