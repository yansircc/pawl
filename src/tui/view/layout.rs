use ratatui::{
    layout::{Constraint, Layout},
    Frame,
};

use crate::tui::state::{AppState, ModalState, ViewMode};

use super::{help_popup, status_bar, task_detail, task_list, tmux_pane};

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Main layout: content area + status bar
    let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(area);

    let content_area = chunks[0];
    let status_area = chunks[1];

    // Render main content based on view
    match &state.view {
        ViewMode::TaskList => {
            task_list::render(frame, content_area, &state.task_list);
        }
        ViewMode::TaskDetail(_) => {
            if let Some(detail) = &state.task_detail {
                task_detail::render(frame, content_area, detail);
            }
        }
        ViewMode::TmuxView(_) => {
            if let Some(tmux) = &state.tmux_view {
                tmux_pane::render(frame, content_area, tmux);
            }
        }
    }

    // Render status bar
    status_bar::render(frame, status_area, state);

    // Render modal on top if present
    if let Some(modal) = &state.modal {
        match modal {
            ModalState::Help => {
                help_popup::render(frame, area, &state.view);
            }
            ModalState::Confirm { .. } => {
                // TODO: implement confirm dialog
            }
        }
    }
}
