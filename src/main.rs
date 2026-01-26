use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
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
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};
use std::{
    collections::HashMap,
    error::Error,
    io::{self, BufRead},
    time::{Duration, Instant},
};
use color_eyre::Result;
use ratatui::{
    crossterm::event::{KeyEventKind},
    layout::{Alignment, Rect},
    style::{Stylize},
    text::Line,
    widgets::{Paragraph},
    DefaultTerminal,
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

    // for line in stdin.lock().lines() {
    //     let line = line.expect("Could not read line from standard in");
    //     println!("{}", line);
    // }

    // let tick_rate = Duration::from_millis(1000);
    // let mut last_tick = Instant::now();

    for line in stdin.lock().lines() {
        let line = line.expect("Could not read line from standard in");
        lines.push(line);
        terminal.draw(|f| ui(f, &lines))?;

        // let timeout = tick_rate
        //     .checked_sub(last_tick.elapsed())
        //     .unwrap_or_else(|| Duration::from_secs(0));

        // if event::poll(timeout)? {
        //     if let Event::Key(key) = event::read()? {
        //         match key.code {
        //             KeyCode::Char('q') | KeyCode::Esc => break,
        //             _ => {}
        //         }
        //     }
        // }

        // if last_tick.elapsed() >= tick_rate {
        //     app.update_stats()?;
        //     last_tick = Instant::now();
        // }
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

    // let header_style = Style::default()
    //     .fg(Color::Yellow)
    //     .add_modifier(Modifier::BOLD);

    // let header_cells = ["Line", "Green", "Blue"]
    //     .iter()
    //     .map(|h| Cell::from(*h).style(header_style));

    // let header = Row::new(header_cells)
    //     .style(Style::default().bg(Color::DarkGray))
    //     .height(1);

    // let processes = app.get_sorted_processes();
    // let rows = processes.iter().map(|p| {
    let rows = lines.iter().map(|line| {
        let cells = vec![Cell::from(line.clone().fg(Color::Green))];
        Row::new(cells).height(1)
    });

    let widths = [
        Constraint::Percentage(80),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
    ];

    let table = Table::new(rows, widths)
        // .header(header)
        .block(Block::default().borders(Borders::ALL).title("LogR"));

    f.render_widget(table, chunks[0]);
}
