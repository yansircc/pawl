use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::Action;
use crate::tui::state::{AppState, ViewMode};

/// Convert a key event to an action based on current state
pub fn handle_key_event(key: KeyEvent, state: &AppState) -> Option<Action> {
    // Handle modal first
    if state.modal.is_some() {
        return handle_modal_key(key);
    }

    // Handle based on current view
    match &state.view {
        ViewMode::TaskList => handle_task_list_key(key, state),
        ViewMode::TaskDetail(_) => handle_task_detail_key(key, state),
        ViewMode::TmuxView(_) => handle_tmux_view_key(key, state),
    }
}

fn handle_modal_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => Some(Action::HideModal),
        KeyCode::Enter => Some(Action::HideModal),
        _ => None,
    }
}

fn handle_task_list_key(key: KeyEvent, state: &AppState) -> Option<Action> {
    // Check for Ctrl+C
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }

    match key.code {
        // Quit
        KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),

        // Navigation
        KeyCode::Char('j') | KeyCode::Down => Some(Action::SelectNext),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::SelectPrev),
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
            if state.task_list.selected_task().is_some() {
                Some(Action::Enter)
            } else {
                None
            }
        }

        // Task operations
        KeyCode::Char('s') => {
            if let Some(task) = state.task_list.selected_task() {
                Some(Action::StartTask(task.name.clone()))
            } else {
                None
            }
        }
        KeyCode::Char('n') => {
            if let Some(task) = state.task_list.selected_task() {
                Some(Action::NextTask(task.name.clone()))
            } else {
                None
            }
        }
        KeyCode::Char('r') => {
            if let Some(task) = state.task_list.selected_task() {
                Some(Action::RetryTask(task.name.clone()))
            } else {
                None
            }
        }
        KeyCode::Char('R') => {
            if let Some(task) = state.task_list.selected_task() {
                Some(Action::ResetTask(task.name.clone()))
            } else {
                None
            }
        }
        KeyCode::Char('x') => {
            if let Some(task) = state.task_list.selected_task() {
                Some(Action::StopTask(task.name.clone()))
            } else {
                None
            }
        }

        // Help
        KeyCode::Char('?') => Some(Action::ShowHelp),

        // Refresh
        KeyCode::Char('g') => Some(Action::Refresh),

        _ => None,
    }
}

fn handle_task_detail_key(key: KeyEvent, state: &AppState) -> Option<Action> {
    // Check for Ctrl+C
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }

    let task_name = match &state.view {
        ViewMode::TaskDetail(name) => name.clone(),
        _ => return None,
    };

    match key.code {
        // Back to list
        KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => {
            Some(Action::Back)
        }

        // Scroll
        KeyCode::Char('j') | KeyCode::Down => Some(Action::ScrollDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::ScrollUp),
        KeyCode::Char('d') | KeyCode::PageDown => Some(Action::PageDown),
        KeyCode::Char('u') | KeyCode::PageUp => Some(Action::PageUp),

        // Enter tmux view for running task
        KeyCode::Enter => Some(Action::SwitchToTmux(task_name.clone())),

        // Task operations
        KeyCode::Char('s') => Some(Action::StartTask(task_name)),
        KeyCode::Char('n') => Some(Action::NextTask(task_name)),
        KeyCode::Char('r') => Some(Action::RetryTask(task_name)),
        KeyCode::Char('R') => Some(Action::ResetTask(task_name)),
        KeyCode::Char('x') => Some(Action::StopTask(task_name)),

        // Help
        KeyCode::Char('?') => Some(Action::ShowHelp),

        // Refresh
        KeyCode::Char('g') => Some(Action::Refresh),

        _ => None,
    }
}

fn handle_tmux_view_key(key: KeyEvent, state: &AppState) -> Option<Action> {
    // Check for Ctrl+C
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }

    let task_name = match &state.view {
        ViewMode::TmuxView(name) => name.clone(),
        _ => return None,
    };

    match key.code {
        // Back to list
        KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => {
            Some(Action::Back)
        }

        // Scroll
        KeyCode::Char('j') | KeyCode::Down => Some(Action::ScrollDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::ScrollUp),
        KeyCode::Char('d') | KeyCode::PageDown => Some(Action::PageDown),
        KeyCode::Char('u') | KeyCode::PageUp => Some(Action::PageUp),

        // Agent operations in tmux view
        KeyCode::Char('D') => Some(Action::DoneTask(task_name.clone())),
        KeyCode::Char('F') => Some(Action::FailTask(task_name.clone())),
        KeyCode::Char('B') => Some(Action::BlockTask(task_name.clone())),

        // Task operations
        KeyCode::Char('n') => Some(Action::NextTask(task_name.clone())),
        KeyCode::Char('r') => Some(Action::RetryTask(task_name.clone())),
        KeyCode::Char('x') => Some(Action::StopTask(task_name)),

        // Help
        KeyCode::Char('?') => Some(Action::ShowHelp),

        // Refresh
        KeyCode::Char('g') => Some(Action::Refresh),

        _ => None,
    }
}
