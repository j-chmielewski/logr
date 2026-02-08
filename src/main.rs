use clap::{ArgAction, Parser};
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers, read,
    },
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
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
    #[arg(short, long)]
    pattern: String,

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    Ok(run(args).await?)
}

async fn run(args: Args) -> Result<(), LogrError> {
    let re = RegexBuilder::new(&args.pattern)
        .case_insensitive(args.ignore_case)
        .build()?;

    let mut terminal = term_init()?;
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines_stream = stdin.lines();
    let mut lines = Vec::new();

    loop {
        if should_exit().await {
            break;
        }

        if let Ok(Ok(Some(line))) =
            timeout(Duration::from_millis(100), lines_stream.next_line()).await
        {
            lines.push(line);
            terminal.draw(|f| ui(f, &lines, &re))?;
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
        Clear(ClearType::All),
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

async fn should_exit() -> bool {
    if crossterm::event::poll(Duration::from_millis(0)).unwrap_or(false) {
        if let Ok(Event::Key(KeyEvent {
            code, modifiers, ..
        })) = read()
        {
            if code == KeyCode::Char('q')
                || (code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL))
            {
                return true;
            }
        }
    }

    false
}

fn ui(f: &mut Frame, lines: &Vec<String>, re: &Regex) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([Constraint::Percentage(100)])
        .split(f.area());

    let rows = lines.iter().map(|line| {
        let caps = re.captures(line);
        if let Some(caps) = caps {
            let mut i = 0;
            let mut spans = Vec::new();
            for cap in caps.iter() {
                let Some(cap) = cap else {
                    continue;
                };
                spans.push(Span::styled(
                    line[i..cap.start()].to_string(),
                    Style::default(),
                ));
                spans.push(Span::styled(
                    line[cap.start()..cap.end()].to_string(),
                    Style::default().fg(Color::Red),
                ));
                i = usize::min(cap.end(), line.len() - 1);
            }
            spans.push(Span::styled(line[i..].to_string(), Style::default()));
            Line::from(spans)
        } else {
            Line::from(line.clone().fg(Color::White))
        }
    });

    let table = Paragraph::new(rows.collect::<Vec<_>>())
        .block(Block::default())
        .block(Block::new().borders(Borders::all()));

    f.render_widget(table, chunks[0]);
}
