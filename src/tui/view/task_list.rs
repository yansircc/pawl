use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::model::TaskStatus;
use crate::tui::state::TaskListState;

use super::style::{format_status, Theme};

pub fn render(frame: &mut Frame, area: Rect, state: &TaskListState) {
    let items: Vec<ListItem> = state
        .tasks
        .iter()
        .map(|task| {
            let (status_text, status_style) = format_status(task.status);

            // Build the line with colored spans
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<15}", truncate(&task.name, 15)),
                    Theme::normal(),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("[{}/{}]", task.current_step + 1, task.total_steps),
                    Theme::dimmed(),
                ),
                Span::raw(" "),
                Span::styled(format!("{:<12}", truncate(&task.step_name, 12)), Theme::normal()),
                Span::raw(" "),
                Span::styled(format!("{:<10}", status_text), status_style),
                Span::raw(" "),
                Span::styled(
                    task.message.clone().unwrap_or_default(),
                    if task.status == TaskStatus::Failed {
                        Theme::status_error()
                    } else {
                        Theme::dimmed()
                    },
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Tasks ")
                .title_style(Theme::title())
                .borders(Borders::ALL)
                .border_style(Theme::border_focused()),
        )
        .highlight_style(Theme::selected())
        .highlight_symbol("> ");

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected));

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}..", &s[..max_len - 2])
    }
}
