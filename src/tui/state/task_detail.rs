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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_detail() -> TaskDetailState {
        TaskDetailState::new(
            "test-task".to_string(),
            "Test description".to_string(),
            vec!["dep1".to_string()],
            TaskStatus::Running,
            1,
            vec![
                StepItem {
                    index: 0,
                    name: "step1".to_string(),
                    step_type: StepType::Normal,
                    status: StepItemStatus::Success,
                },
                StepItem {
                    index: 1,
                    name: "step2".to_string(),
                    step_type: StepType::Checkpoint,
                    status: StepItemStatus::Current,
                },
            ],
            Some("Waiting for input".to_string()),
        )
    }

    #[test]
    fn test_new() {
        let detail = make_detail();
        assert_eq!(detail.name, "test-task");
        assert_eq!(detail.description, "Test description");
        assert_eq!(detail.depends, vec!["dep1"]);
        assert_eq!(detail.status, TaskStatus::Running);
        assert_eq!(detail.current_step, 1);
        assert_eq!(detail.steps.len(), 2);
        assert_eq!(detail.message, Some("Waiting for input".to_string()));
        assert_eq!(detail.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_up() {
        let mut detail = make_detail();
        detail.scroll_offset = 10;

        let detail = detail.scroll_up(3);
        assert_eq!(detail.scroll_offset, 7);

        let detail = detail.scroll_up(10);
        assert_eq!(detail.scroll_offset, 0);

        // Can't go below 0
        let detail = detail.scroll_up(1);
        assert_eq!(detail.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_down() {
        let detail = make_detail();
        assert_eq!(detail.scroll_offset, 0);

        let detail = detail.scroll_down(5, 20);
        assert_eq!(detail.scroll_offset, 5);

        let detail = detail.scroll_down(10, 20);
        assert_eq!(detail.scroll_offset, 15);

        // Can't exceed max_lines - 1
        let detail = detail.scroll_down(10, 20);
        assert_eq!(detail.scroll_offset, 19);
    }

    #[test]
    fn test_step_item_status_from() {
        assert_eq!(
            StepItemStatus::from(StepStatus::Success),
            StepItemStatus::Success
        );
        assert_eq!(
            StepItemStatus::from(StepStatus::Failed),
            StepItemStatus::Failed
        );
        assert_eq!(
            StepItemStatus::from(StepStatus::Skipped),
            StepItemStatus::Skipped
        );
        assert_eq!(
            StepItemStatus::from(StepStatus::Blocked),
            StepItemStatus::Blocked
        );
    }
}
