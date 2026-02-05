use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::tui::state::ViewMode;

use super::style::Theme;

pub fn render(frame: &mut Frame, area: Rect, view: &ViewMode) {
    let popup_area = centered_rect(60, 80, area);

    // Clear background
    frame.render_widget(Clear, popup_area);

    let title = match view {
        ViewMode::TaskList => " Help - Task List ",
        ViewMode::TaskDetail(_) => " Help - Task Detail ",
        ViewMode::TmuxView(_) => " Help - Tmux View ",
    };

    let help_items = get_help_items(view);

    let lines: Vec<Line> = help_items
        .iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(format!("{:<12}", key), Theme::help_key()),
                Span::styled(*desc, Theme::help_desc()),
            ])
        })
        .collect();

    let block = Block::default()
        .title(title)
        .title_style(Theme::title())
        .borders(Borders::ALL)
        .border_style(Theme::border_focused());

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup_area);
}

fn get_help_items(view: &ViewMode) -> Vec<(&'static str, &'static str)> {
    let mut items = vec![
        ("q/Esc", "Quit / Back"),
        ("?", "Toggle help"),
        ("g", "Refresh"),
    ];

    match view {
        ViewMode::TaskList => {
            items.extend([
                ("j/Down", "Move down"),
                ("k/Up", "Move up"),
                ("Enter", "View detail / tmux"),
                ("", ""),
                ("s", "Start task"),
                ("n", "Next (checkpoint)"),
                ("r", "Retry step"),
                ("S", "Skip step"),
                ("R", "Reset task"),
                ("x", "Stop task"),
            ]);
        }
        ViewMode::TaskDetail(_) => {
            items.extend([
                ("j/Down", "Scroll down"),
                ("k/Up", "Scroll up"),
                ("Enter", "View tmux (if running)"),
                ("", ""),
                ("s", "Start task"),
                ("n", "Next (checkpoint)"),
                ("r", "Retry step"),
                ("S", "Skip step"),
                ("R", "Reset task"),
                ("x", "Stop task"),
            ]);
        }
        ViewMode::TmuxView(_) => {
            items.extend([
                ("j/Down", "Scroll down"),
                ("k/Up", "Scroll up"),
                ("d/PgDn", "Page down"),
                ("u/PgUp", "Page up"),
                ("", ""),
                ("D", "Mark done"),
                ("F", "Mark failed"),
                ("B", "Mark blocked"),
                ("n", "Next (checkpoint)"),
                ("r", "Retry step"),
                ("x", "Stop task"),
            ]);
        }
    }

    items
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
