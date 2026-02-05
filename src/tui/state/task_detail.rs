use crate::model::{StepStatus, TaskStatus};

/// Step information in task detail
#[derive(Debug, Clone)]
pub struct StepItem {
    pub index: usize,
    pub name: String,
    pub step_type: StepType,
    pub status: StepItemStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepType {
    Normal,
    Checkpoint,
    InWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepItemStatus {
    Pending,
    Current,
    Success,
    Failed,
    Skipped,
    Blocked,
}

impl From<StepStatus> for StepItemStatus {
    fn from(status: StepStatus) -> Self {
        match status {
            StepStatus::Success => StepItemStatus::Success,
            StepStatus::Failed => StepItemStatus::Failed,
            StepStatus::Skipped => StepItemStatus::Skipped,
            StepStatus::Blocked => StepItemStatus::Blocked,
        }
    }
}

/// State for the task detail view
#[derive(Debug, Clone)]
pub struct TaskDetailState {
    pub name: String,
    pub description: String,
    pub depends: Vec<String>,
    pub status: TaskStatus,
    pub current_step: usize,
    pub steps: Vec<StepItem>,
    pub message: Option<String>,
    pub scroll_offset: usize,
}

impl TaskDetailState {
    pub fn new(
        name: String,
        description: String,
        depends: Vec<String>,
        status: TaskStatus,
        current_step: usize,
        steps: Vec<StepItem>,
        message: Option<String>,
    ) -> Self {
        Self {
            name,
            description,
            depends,
            status,
            current_step,
            steps,
            message,
            scroll_offset: 0,
        }
    }

    pub fn scroll_up(&self, lines: usize) -> Self {
        Self {
            scroll_offset: self.scroll_offset.saturating_sub(lines),
            ..self.clone()
        }
    }

    pub fn scroll_down(&self, lines: usize, max_lines: usize) -> Self {
        let max_offset = max_lines.saturating_sub(1);
        Self {
            scroll_offset: (self.scroll_offset + lines).min(max_offset),
            ..self.clone()
        }
    }
}
