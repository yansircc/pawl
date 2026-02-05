use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::state::TmuxViewState;

use super::style::Theme;

pub fn render(frame: &mut Frame, area: Rect, state: &TmuxViewState) {
    let title = if state.window_exists {
        format!(" Tmux: {} (live) ", state.task_name)
    } else {
        format!(" Tmux: {} (disconnected) ", state.task_name)
    };

    let lines: Vec<Line> = state
        .content
        .lines()
        .skip(state.scroll_offset)
        .map(|line| Line::from(Span::raw(line)))
        .collect();

    let scroll_indicator = if state.auto_scroll {
        " [auto-scroll] ".to_string()
    } else {
        format!(
            " [line {}/{}] ",
            state.scroll_offset + 1,
            state.content_lines().len().max(1)
        )
    };

    let block = Block::default()
        .title(title)
        .title_style(if state.window_exists {
            Theme::title()
        } else {
            Theme::dimmed()
        })
        .title_bottom(scroll_indicator)
        .borders(Borders::ALL)
        .border_style(if state.window_exists {
            Theme::border_focused()
        } else {
            Theme::dimmed()
        });

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}
