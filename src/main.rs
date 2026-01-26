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
    style::{Color, Stylize},
    widgets::{Block, Cell, Row, Table},
};
use std::{
    error::Error,
    io::{self, BufRead},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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
        terminal.draw(|f| ui(f, &lines))?;
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

fn ui(f: &mut Frame, lines: &Vec<String>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Percentage(100)])
        .split(f.area());

    let rows = lines.iter().map(|line| {
        let cells = vec![Cell::from(line.clone().fg(Color::Green))];
        Row::new(cells).height(1)
    });

    let widths = [
        Constraint::Percentage(100),
    ];

    let table = Table::new(rows, widths).block(Block::default());

    f.render_widget(table, chunks[0]);
}
