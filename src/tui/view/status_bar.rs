use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::tui::state::{AppState, ViewMode};

use super::style::Theme;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let left_text = match &state.view {
        ViewMode::TaskList => {
            let count = state.task_list.tasks.len();
            format!(" {} tasks ", count)
        }
        ViewMode::TaskDetail(name) => format!(" Task: {} ", name),
        ViewMode::TmuxView(name) => format!(" Tmux: {} ", name),
    };

    let help_hint = " Press ? for help ";

    let message = state
        .status_message
        .as_ref()
        .map(|m| m.text.as_str())
        .unwrap_or("");

    let message_style = if state
        .status_message
        .as_ref()
        .map(|m| m.is_error)
        .unwrap_or(false)
    {
        Theme::status_error()
    } else {
        Theme::status_message()
    };

    // Calculate spacing
    let total_width = area.width as usize;
    let left_len = left_text.len();
    let right_len = help_hint.len();
    let msg_len = message.len();
    let padding = total_width
        .saturating_sub(left_len)
        .saturating_sub(right_len)
        .saturating_sub(msg_len);

    let line = Line::from(vec![
        Span::styled(left_text, Theme::status_bar()),
        Span::styled(message, message_style),
        Span::styled(" ".repeat(padding), Theme::status_bar()),
        Span::styled(help_hint, Theme::dimmed()),
    ]);

    let paragraph = Paragraph::new(line).style(Theme::status_bar());
    frame.render_widget(paragraph, area);
}
