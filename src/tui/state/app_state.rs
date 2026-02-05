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
    Confirm { title: String, message: String },
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
