use crate::model::TaskStatus;

/// Task item displayed in the list
#[derive(Debug, Clone)]
pub struct TaskItem {
    pub name: String,
    pub status: TaskStatus,
    pub current_step: usize,
    pub total_steps: usize,
    pub step_name: String,
    #[allow(dead_code)] // Reserved for dependency display
    pub blocked_by: Vec<String>,
    pub message: Option<String>,
}

/// State for the task list view
#[derive(Debug, Clone)]
pub struct TaskListState {
    pub tasks: Vec<TaskItem>,
    pub selected: usize,
}

impl TaskListState {
    #[allow(dead_code)] // Used in tests
    pub fn new(tasks: Vec<TaskItem>) -> Self {
        Self { tasks, selected: 0 }
    }

    pub fn empty() -> Self {
        Self {
            tasks: Vec::new(),
            selected: 0,
        }
    }

    pub fn select_next(&self) -> Self {
        let selected = if self.tasks.is_empty() {
            0
        } else if self.selected >= self.tasks.len() - 1 {
            self.selected
        } else {
            self.selected + 1
        };
        Self {
            tasks: self.tasks.clone(),
            selected,
        }
    }

    pub fn select_prev(&self) -> Self {
        let selected = if self.selected > 0 {
            self.selected - 1
        } else {
            0
        };
        Self {
            tasks: self.tasks.clone(),
            selected,
        }
    }

    pub fn selected_task(&self) -> Option<&TaskItem> {
        self.tasks.get(self.selected)
    }

    pub fn update_tasks(&self, tasks: Vec<TaskItem>) -> Self {
        let selected = self.selected.min(tasks.len().saturating_sub(1));
        Self { tasks, selected }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_new_empty() {
        let state = TaskListState::empty();
        assert!(state.tasks.is_empty());
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_navigation() {
        let tasks = vec![make_task("task1"), make_task("task2"), make_task("task3")];
        let state = TaskListState::new(tasks);
        assert_eq!(state.selected, 0);

        let state = state.select_next();
        assert_eq!(state.selected, 1);

        let state = state.select_next();
        assert_eq!(state.selected, 2);

        // Can't go past last
        let state = state.select_next();
        assert_eq!(state.selected, 2);

        let state = state.select_prev();
        assert_eq!(state.selected, 1);

        let state = state.select_prev();
        assert_eq!(state.selected, 0);

        // Can't go before first
        let state = state.select_prev();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_selected_task() {
        let tasks = vec![make_task("task1"), make_task("task2")];
        let state = TaskListState::new(tasks);
        assert_eq!(state.selected_task().unwrap().name, "task1");

        let state = state.select_next();
        assert_eq!(state.selected_task().unwrap().name, "task2");
    }

    #[test]
    fn test_update_preserves_selection() {
        let tasks = vec![make_task("task1"), make_task("task2"), make_task("task3")];
        let state = TaskListState::new(tasks);
        let state = state.select_next().select_next(); // selected = 2

        // Update with same number of tasks
        let new_tasks = vec![make_task("a"), make_task("b"), make_task("c")];
        let state = state.update_tasks(new_tasks);
        assert_eq!(state.selected, 2);

        // Update with fewer tasks
        let new_tasks = vec![make_task("x")];
        let state = state.update_tasks(new_tasks);
        assert_eq!(state.selected, 0);
    }
}
