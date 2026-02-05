use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::style::Theme;

pub fn render(frame: &mut Frame, area: Rect, title: &str, message: &str) {
    let popup_area = centered_rect(50, 30, area);

    // Clear background
    frame.render_widget(Clear, popup_area);

    let title = format!(" {} ", title);

    // Build content lines
    let lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(message),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Y]", Theme::help_key()),
            Span::styled(" Yes   ", Theme::help_desc()),
            Span::styled("[N]", Theme::help_key()),
            Span::styled(" No", Theme::help_desc()),
        ]),
    ];

    let block = Block::default()
        .title(title)
        .title_style(Theme::title())
        .borders(Borders::ALL)
        .border_style(Theme::border_focused());

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, popup_area);
}

/// Create a centered rect using given percentage of the available area
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
