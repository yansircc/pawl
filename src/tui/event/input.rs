use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::Action;
use crate::tui::state::{AppState, ViewMode};

/// Convert a key event to an action based on current state
pub fn handle_key_event(key: KeyEvent, state: &AppState) -> Option<Action> {
    // Handle modal first
    if let Some(modal) = &state.modal {
        return handle_modal_key(key, modal);
    }

    // Handle based on current view
    match &state.view {
        ViewMode::TaskList => handle_task_list_key(key, state),
        ViewMode::TaskDetail(_) => handle_task_detail_key(key, state),
        ViewMode::TmuxView(_) => handle_tmux_view_key(key, state),
    }
}

use crate::tui::state::ModalState;

fn handle_modal_key(key: KeyEvent, modal: &ModalState) -> Option<Action> {
    match modal {
        ModalState::Help => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => Some(Action::HideModal),
            KeyCode::Enter => Some(Action::HideModal),
            _ => None,
        },
        ModalState::Confirm { .. } => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Some(Action::ConfirmYes),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(Action::ConfirmNo),
            _ => None,
        },
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
                Some(Action::ShowConfirm {
                    title: "Reset Task".to_string(),
                    message: format!("Reset task '{}'? All progress will be lost.", task.name),
                    on_confirm: Box::new(Action::ResetTask(task.name.clone())),
                })
            } else {
                None
            }
        }
        KeyCode::Char('x') => {
            if let Some(task) = state.task_list.selected_task() {
                Some(Action::ShowConfirm {
                    title: "Stop Task".to_string(),
                    message: format!("Stop task '{}'?", task.name),
                    on_confirm: Box::new(Action::StopTask(task.name.clone())),
                })
            } else {
                None
            }
        }
        KeyCode::Char('S') => {
            if let Some(task) = state.task_list.selected_task() {
                Some(Action::SkipTask(task.name.clone()))
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
        KeyCode::Char('n') => Some(Action::NextTask(task_name.clone())),
        KeyCode::Char('r') => Some(Action::RetryTask(task_name.clone())),
        KeyCode::Char('R') => Some(Action::ShowConfirm {
            title: "Reset Task".to_string(),
            message: format!("Reset task '{}'? All progress will be lost.", task_name),
            on_confirm: Box::new(Action::ResetTask(task_name.clone())),
        }),
        KeyCode::Char('x') => Some(Action::ShowConfirm {
            title: "Stop Task".to_string(),
            message: format!("Stop task '{}'?", task_name),
            on_confirm: Box::new(Action::StopTask(task_name.clone())),
        }),
        KeyCode::Char('S') => Some(Action::SkipTask(task_name)),

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
        KeyCode::Char('x') => Some(Action::ShowConfirm {
            title: "Stop Task".to_string(),
            message: format!("Stop task '{}'?", task_name),
            on_confirm: Box::new(Action::StopTask(task_name)),
        }),

        // Help
        KeyCode::Char('?') => Some(Action::ShowHelp),

        // Refresh
        KeyCode::Char('g') => Some(Action::Refresh),

        _ => None,
    }
}
