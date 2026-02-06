use anyhow::Result;

use crate::tui::state::{TaskDetailState, TaskItem};

/// Result of capturing tmux pane content
#[derive(Debug, Clone)]
pub struct TmuxCaptureResult {
    pub content: String,
    pub window_exists: bool,
}

/// Task operation that can be executed
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskAction {
    Start(String),
    Stop(String),
    Reset(String),
    Next(String),
    Retry(String),
    Skip(String),
    Done(String),
    Fail(String),
}

/// Trait for loading data and executing actions
/// This allows mocking for tests
pub trait DataProvider: Send + Sync {
    /// Load all tasks
    fn load_tasks(&self) -> Result<Vec<TaskItem>>;

    /// Load task detail
    fn load_task_detail(&self, name: &str) -> Result<TaskDetailState>;

    /// Capture tmux pane content
    fn capture_tmux(&self, task_name: &str, lines: usize) -> Result<TmuxCaptureResult>;

    /// Execute a task action
    fn execute_action(&self, action: &TaskAction) -> Result<()>;
}
