use std::time::Instant;

use super::{TaskDetailState, TaskListState, TmuxViewState};

/// Current view mode
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewMode {
    TaskList,
    TaskDetail(String),
    TmuxView(String),
}

/// Modal dialog state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalState {
    Help,
    Confirm {
        title: String,
        message: String,
        on_confirm: Box<crate::tui::event::Action>,
    },
}

/// Status message with expiration
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
    pub created_at: Instant,
}

impl StatusMessage {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
            created_at: Instant::now(),
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
            created_at: Instant::now(),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().as_secs() > 3
    }
}

/// Root application state
#[derive(Debug, Clone)]
pub struct AppState {
    pub view: ViewMode,
    pub task_list: TaskListState,
    pub task_detail: Option<TaskDetailState>,
    pub tmux_view: Option<TmuxViewState>,
    pub modal: Option<ModalState>,
    pub status_message: Option<StatusMessage>,
    pub should_quit: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            view: ViewMode::TaskList,
            task_list: TaskListState::empty(),
            task_detail: None,
            tmux_view: None,
            modal: None,
            status_message: None,
            should_quit: false,
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)] // Used in tests
    pub fn with_tasks(tasks: Vec<super::TaskItem>) -> Self {
        Self {
            task_list: TaskListState::new(tasks),
            ..Self::default()
        }
    }

    pub fn set_status(&self, message: StatusMessage) -> Self {
        Self {
            status_message: Some(message),
            ..self.clone()
        }
    }

    pub fn clear_expired_status(&self) -> Self {
        if let Some(ref msg) = self.status_message {
            if msg.is_expired() {
                return Self {
                    status_message: None,
                    ..self.clone()
                };
            }
        }
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskStatus;
    use crate::tui::event::Action;
    use crate::tui::state::TaskItem;

    fn make_task(name: &str) -> TaskItem {
        TaskItem {
            name: name.to_string(),
            status: TaskStatus::Pending,
            current_step: 0,
            total_steps: 3,
            step_name: "Init".to_string(),
            blocked_by: vec![],
            message: None,
        }
    }

    #[test]
    fn test_default() {
        let state = AppState::default();
        assert_eq!(state.view, ViewMode::TaskList);
        assert!(state.task_list.tasks.is_empty());
        assert!(state.task_detail.is_none());
        assert!(state.tmux_view.is_none());
        assert!(state.modal.is_none());
        assert!(state.status_message.is_none());
        assert!(!state.should_quit);
    }

    #[test]
    fn test_with_tasks() {
        let tasks = vec![make_task("task1"), make_task("task2")];
        let state = AppState::with_tasks(tasks);
        assert_eq!(state.task_list.tasks.len(), 2);
        assert_eq!(state.task_list.selected, 0);
    }

    #[test]
    fn test_set_status() {
        let state = AppState::default();
        assert!(state.status_message.is_none());

        let state = state.set_status(StatusMessage::info("Test message"));
        assert!(state.status_message.is_some());
        assert_eq!(state.status_message.as_ref().unwrap().text, "Test message");
        assert!(!state.status_message.as_ref().unwrap().is_error);

        let state = state.set_status(StatusMessage::error("Error!"));
        assert!(state.status_message.as_ref().unwrap().is_error);
    }

    #[test]
    fn test_status_message_expiration() {
        let msg = StatusMessage::info("Test");
        assert!(!msg.is_expired());
        // Note: We can't easily test expiration without waiting 3+ seconds
        // so we just verify the function exists and works for non-expired
    }

    #[test]
    fn test_clear_expired_status_not_expired() {
        let state = AppState::default();
        let state = state.set_status(StatusMessage::info("Test"));

        // Message is fresh, should not be cleared
        let state = state.clear_expired_status();
        assert!(state.status_message.is_some());
    }

    #[test]
    fn test_view_mode_equality() {
        assert_eq!(ViewMode::TaskList, ViewMode::TaskList);
        assert_eq!(
            ViewMode::TaskDetail("task1".to_string()),
            ViewMode::TaskDetail("task1".to_string())
        );
        assert_ne!(
            ViewMode::TaskDetail("task1".to_string()),
            ViewMode::TaskDetail("task2".to_string())
        );
        assert_ne!(ViewMode::TaskList, ViewMode::TmuxView("task".to_string()));
    }

    #[test]
    fn test_modal_state_equality() {
        assert_eq!(ModalState::Help, ModalState::Help);

        let confirm1 = ModalState::Confirm {
            title: "Test".to_string(),
            message: "msg".to_string(),
            on_confirm: Box::new(Action::Quit),
        };
        let confirm2 = ModalState::Confirm {
            title: "Test".to_string(),
            message: "msg".to_string(),
            on_confirm: Box::new(Action::Quit),
        };
        assert_eq!(confirm1, confirm2);

        assert_ne!(ModalState::Help, confirm1);
    }
}
