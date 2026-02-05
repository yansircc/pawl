use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::tui::state::task_detail::{StepItemStatus, StepType};
use crate::tui::state::TaskDetailState;

use super::style::{format_status, status_marker, Theme};

pub fn render(frame: &mut Frame, area: Rect, state: &TaskDetailState) {
    let chunks = Layout::vertical([
        Constraint::Length(7), // Header section
        Constraint::Min(5),    // Workflow steps
    ])
    .split(area);

    render_header(frame, chunks[0], state);
    render_steps(frame, chunks[1], state);
}

fn render_header(frame: &mut Frame, area: Rect, state: &TaskDetailState) {
    let (status_text, status_style) = format_status(state.status);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Task: ", Theme::dimmed()),
            Span::styled(&state.name, Theme::highlight()),
        ]),
        Line::from(vec![
            Span::styled("Status: ", Theme::dimmed()),
            Span::styled(status_text, status_style),
        ]),
        Line::from(vec![
            Span::styled("Progress: ", Theme::dimmed()),
            Span::raw(format!(
                "{}/{}",
                state.current_step + 1,
                state.steps.len()
            )),
        ]),
    ];

    if !state.depends.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Depends: ", Theme::dimmed()),
            Span::raw(state.depends.join(", ")),
        ]));
    }

    if let Some(msg) = &state.message {
        lines.push(Line::from(vec![
            Span::styled("Message: ", Theme::dimmed()),
            Span::styled(msg, Theme::status_error()),
        ]));
    }

    let block = Block::default()
        .title(" Task Detail ")
        .title_style(Theme::title())
        .borders(Borders::ALL)
        .border_style(Theme::border_focused());

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_steps(frame: &mut Frame, area: Rect, state: &TaskDetailState) {
    let items: Vec<ListItem> = state
        .steps
        .iter()
        .map(|step| {
            let marker = status_marker(step.status);
            let marker_style = Style::default().fg(Theme::step_status_color(step.status));

            let step_type_str = match step.step_type {
                StepType::Normal => "",
                StepType::Checkpoint => " (checkpoint)",
                StepType::InWindow => " (in_window)",
            };

            let line = Line::from(vec![
                Span::styled(format!(" {} ", marker), marker_style),
                Span::styled(
                    format!("{}. ", step.index + 1),
                    Theme::dimmed(),
                ),
                Span::styled(
                    &step.name,
                    if step.status == StepItemStatus::Current {
                        Theme::highlight()
                    } else {
                        Theme::normal()
                    },
                ),
                Span::styled(step_type_str, Theme::dimmed()),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Workflow Steps ")
            .title_style(Theme::title())
            .borders(Borders::ALL)
            .border_style(Theme::border()),
    );

    frame.render_widget(list, area);
}
