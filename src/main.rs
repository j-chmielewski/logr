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
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
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

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, num_args = 1.., value_delimiter = ',')]
    patterns: Vec<String>,

    #[arg(short, long, action = ArgAction::SetTrue)]
    ignore_case: bool,
}

#[derive(Error, Debug)]
enum LogrError {
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    RegexError(#[from] regex::Error),
}

struct AppState {
    patterns: Vec<String>,
    selected: usize,
    dialog_open: bool,
    input: String,
    pattern_error: Option<String>,
    regexes: Vec<Regex>,
    ignore_case: bool,
    scroll: usize,
    follow: bool,
    wrap: bool,
}

const PATTERN_COLORS: [Color; 10] = [
    Color::Red,
    Color::Green,
    Color::Blue,
    Color::Yellow,
    Color::Magenta,
    Color::Cyan,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
];

fn pattern_color(index: usize) -> Color {
    PATTERN_COLORS[index % PATTERN_COLORS.len()]
}

fn max_start(total_lines: usize, view_height: usize) -> usize {
    if view_height == 0 {
        0
    } else {
        total_lines.saturating_sub(view_height)
    }
}

const TICK_RATE: Duration = Duration::from_millis(20);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    Ok(run(args).await?)
}

async fn run(args: Args) -> Result<(), LogrError> {
    let mut regexes = Vec::new();
    for pattern in &args.patterns {
        regexes.push(build_regex(pattern, args.ignore_case)?);
    }
    let mut app = AppState {
        patterns: args.patterns,
        selected: 0,
        dialog_open: false,
        input: String::new(),
        pattern_error: None,
        regexes,
        ignore_case: args.ignore_case,
        scroll: 0,
        follow: true,
        wrap: false,
    };

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

    term_deinit(terminal)?;

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

fn term_deinit(mut terminal: LogrTerminal) -> Result<(), io::Error> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()
}

fn build_regex(pattern: &str, ignore_case: bool) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .build()
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
                            match build_regex(&app.input, app.ignore_case) {
                                Ok(regex) => {
                                    app.patterns.push(app.input.clone());
                                    app.regexes.push(regex);
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
                    KeyCode::Delete => {
                        if app.selected < app.patterns.len() {
                            app.patterns.remove(app.selected);
                            app.regexes.remove(app.selected);
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

fn ui(f: &mut Frame, lines: &[String], app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([Constraint::Percentage(100)])
        .split(f.area());

    let content_height = chunks[0].height.saturating_sub(2) as usize;
    let total_lines = lines.len();
    let max_start = max_start(total_lines, content_height);
    let start = if app.follow {
        max_start
    } else {
        app.scroll.min(max_start)
    };
    let rows = lines[start..]
        .iter()
        .map(|line| highlight_line(line, &app.regexes));

    let mut table = Paragraph::new(rows.collect::<Vec<_>>())
        .block(Block::default())
        .block(Block::new().borders(Borders::all()));

    if app.wrap {
        table = table.wrap(Wrap { trim: false });
    }

    f.render_widget(table, chunks[0]);

    if total_lines > 0 && start < max_start {
        let current_line = start.saturating_add(1);
        let percent = (current_line * 100) / total_lines;
        let status = format!("[{current_line}/{total_lines} ({percent}%)]");
        let width = status.len() as u16;
        let max_width = chunks[0].width.saturating_sub(2);
        if width <= max_width && chunks[0].height > 0 {
            let x = chunks[0].x + chunks[0].width.saturating_sub(width + 1);
            let y = chunks[0].y + chunks[0].height.saturating_sub(1);
            let area = Rect {
                x,
                y,
                width,
                height: 1,
            };
            let status_line = Paragraph::new(status).style(Style::default().fg(Color::Yellow));
            f.render_widget(status_line, area);
        }
    }

    if app.dialog_open {
        let area = centered_rect(80, 60, f.area());
        f.render_widget(Clear, area);
        let mut dialog_lines = Vec::new();

        for (i, pattern) in app.patterns.iter().enumerate() {
            let prefix = if app.selected == i { "> " } else { "  " };
            dialog_lines.push(Line::from(Span::styled(
                format!("{prefix}{pattern}"),
                Style::default().fg(pattern_color(i)),
            )));
        }

        if let Some(err) = &app.pattern_error {
            dialog_lines.push(Line::from(Span::styled(
                err.clone(),
                Style::default().fg(Color::Red),
            )));
        }

        let input_style = if app.selected == app.patterns.len() {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::White)
        };
        dialog_lines.push(Line::from(Span::styled(
            format!(
                "{}+ {}",
                if app.selected == app.patterns.len() {
                    "> "
                } else {
                    "  "
                },
                app.input
            ),
            input_style,
        )));

        let dialog = Paragraph::new(dialog_lines).block(
            Block::default()
                .borders(Borders::all())
                .title("Patterns (Enter: add, Del: delete, Esc: close)"),
        );

        f.render_widget(dialog, area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

fn highlight_line<'a>(line: &'a str, regexes: &[Regex]) -> Line<'a> {
    let mut ranges: Vec<(usize, usize, usize, Color)> = Vec::new();
    for (index, regex) in regexes.iter().enumerate() {
        let color = pattern_color(index);
        for mat in regex.find_iter(line) {
            let start = mat.start();
            let end = mat.end();
            if start < end {
                ranges.push((start, end, index, color));
            }
        }
    }

    if ranges.is_empty() {
        return Line::from(line.to_string().fg(Color::White));
    }

    ranges.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.2.cmp(&b.2)));
    let mut spans = Vec::new();
    let mut cursor = 0;

    for (mut start, end, _, color) in ranges {
        if end <= cursor {
            continue;
        }
        if start < cursor {
            start = cursor;
        }
        if cursor < start {
            spans.push(Span::styled(
                line[cursor..start].to_string(),
                Style::default(),
            ));
        }
        spans.push(Span::styled(
            line[start..end].to_string(),
            Style::default().fg(color),
        ));
        cursor = end;
    }

    if cursor < line.len() {
        spans.push(Span::styled(line[cursor..].to_string(), Style::default()));
    }

    Line::from(spans)
}
