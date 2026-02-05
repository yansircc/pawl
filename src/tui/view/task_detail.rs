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
    // Calculate header height based on content
    let header_height = 4 + state.depends.is_empty().then_some(0).unwrap_or(1)
        + state.message.is_some().then_some(1).unwrap_or(0);

    // Show description if present
    if !state.description.is_empty() {
        let desc_lines = state.description.lines().count().min(5) as u16 + 2; // +2 for border
        let chunks = Layout::vertical([
            Constraint::Length(header_height as u16 + 2), // +2 for border
            Constraint::Length(desc_lines),
            Constraint::Min(5), // Workflow steps
        ])
        .split(area);

        render_header(frame, chunks[0], state);
        render_description(frame, chunks[1], state);
        render_steps(frame, chunks[2], state);
    } else {
        let chunks = Layout::vertical([
            Constraint::Length(header_height as u16 + 2), // +2 for border
            Constraint::Min(5), // Workflow steps
        ])
        .split(area);

        render_header(frame, chunks[0], state);
        render_steps(frame, chunks[1], state);
    }
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

fn render_description(frame: &mut Frame, area: Rect, state: &TaskDetailState) {
    let lines: Vec<Line> = state
        .description
        .lines()
        .take(5)
        .map(|line| Line::from(Span::raw(line)))
        .collect();

    let block = Block::default()
        .title(" Description ")
        .title_style(Theme::dimmed())
        .borders(Borders::ALL)
        .border_style(Theme::border());

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
