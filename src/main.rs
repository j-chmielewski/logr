use clap::{ArgAction, Parser};
use color_eyre::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, poll, read},
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
    error::Error,
    io,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    signal, time::timeout,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    pattern: String,

    #[arg(short, long, action = ArgAction::SetTrue)]
    ignore_case: bool,
}

static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

async fn setup_signal_handlers() {
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for ctrl-c");
        SHOULD_EXIT.store(true, Ordering::SeqCst);
    });
}

async fn check_exit() -> bool {
    if SHOULD_EXIT.load(Ordering::SeqCst) {
        return true;
    }

    if poll(Duration::from_millis(0)).unwrap_or(false) {
        if let Ok(Event::Key(KeyEvent { code, .. })) = read() {
            if code == KeyCode::Char('q') {
                return true;
            }
        }
    }

    false
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    setup_signal_handlers().await;

    let re = RegexBuilder::new(&args.pattern)
        .case_insensitive(args.ignore_case)
        .build()
        .expect("Invalid regex pattern");

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        Clear(ClearType::All),
        EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = Vec::new();
    let mut lines_stream = stdin.lines();

    loop {
        if check_exit().await {
            break;
        }

        let line = timeout(Duration::from_millis(100), lines_stream.next_line()).await;

        if check_exit().await {
            break;
        }

        match line {
            Ok(Ok(Some(l))) => {
                lines.push(l);
                terminal.draw(|f| ui(f, &lines, &re))?;
            }
            Ok(Ok(None)) | Err(_) => {
                terminal.draw(|f| ui(f, &lines, &re))?;
            }
            _ => {}
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
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
