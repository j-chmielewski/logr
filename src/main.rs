use clap::{ArgAction, Parser};
use color_eyre::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
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
    io::{self, BufRead},
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    pattern: String,

    #[arg(short, long, action = ArgAction::SetTrue)]
    ignore_case: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

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
    let stdin = io::stdin();
    let mut lines = Vec::new();

    for line in stdin.lock().lines() {
        let line = line.expect("Could not read line from standard in");
        lines.push(line);
        terminal.draw(|f| ui(f, &lines, &re))?;
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
