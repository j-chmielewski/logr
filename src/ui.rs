use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use regex::Regex;

use crate::{AppState, max_start};

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

pub(crate) fn ui(f: &mut Frame, lines: &[String], app: &AppState) {
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

    if chunks[0].height > 0 {
        let hint = "p: patterns  w: wrap  q: quit";
        let hint_width = hint.len() as u16;
        let max_width = chunks[0].width.saturating_sub(2);
        if hint_width <= max_width {
            let area = Rect {
                x: chunks[0].x + 1,
                y: chunks[0].y + chunks[0].height.saturating_sub(1),
                width: hint_width,
                height: 1,
            };
            let hint_line = Paragraph::new(hint).style(Style::default().fg(Color::DarkGray));
            f.render_widget(hint_line, area);
        }
    }

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
