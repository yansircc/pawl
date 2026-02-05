use super::{AppState, ModalState, TmuxViewState, ViewMode};
use crate::tui::event::Action;

/// Pure function that transforms state based on action
pub fn reduce(state: AppState, action: Action) -> AppState {
    match action {
        Action::Quit => AppState {
            should_quit: true,
            ..state
        },

        Action::Back => match &state.view {
            ViewMode::TaskList => state, // Already at list, no-op
            ViewMode::TaskDetail(_) | ViewMode::TmuxView(_) => AppState {
                view: ViewMode::TaskList,
                task_detail: None,
                tmux_view: None,
                ..state
            },
        },

        Action::SelectNext => AppState {
            task_list: state.task_list.select_next(),
            ..state
        },

        Action::SelectPrev => AppState {
            task_list: state.task_list.select_prev(),
            ..state
        },

        Action::Enter => {
            if let Some(task) = state.task_list.selected_task() {
                AppState {
                    view: ViewMode::TaskDetail(task.name.clone()),
                    ..state
                }
            } else {
                state
            }
        }

        Action::ScrollUp => match &state.view {
            ViewMode::TaskDetail(_) => {
                if let Some(detail) = &state.task_detail {
                    AppState {
                        task_detail: Some(detail.scroll_up(1)),
                        ..state
                    }
                } else {
                    state
                }
            }
            ViewMode::TmuxView(_) => {
                if let Some(tmux) = &state.tmux_view {
                    AppState {
                        tmux_view: Some(tmux.scroll_up(1)),
                        ..state
                    }
                } else {
                    state
                }
            }
            _ => state,
        },

        Action::ScrollDown => match &state.view {
            ViewMode::TaskDetail(_) => {
                if let Some(detail) = &state.task_detail {
                    // Use a reasonable max for scroll
                    AppState {
                        task_detail: Some(detail.scroll_down(1, 100)),
                        ..state
                    }
                } else {
                    state
                }
            }
            ViewMode::TmuxView(_) => {
                if let Some(tmux) = &state.tmux_view {
                    AppState {
                        tmux_view: Some(tmux.scroll_down(1)),
                        ..state
                    }
                } else {
                    state
                }
            }
            _ => state,
        },

        Action::PageUp => match &state.view {
            ViewMode::TaskDetail(_) => {
                if let Some(detail) = &state.task_detail {
                    AppState {
                        task_detail: Some(detail.scroll_up(10)),
                        ..state
                    }
                } else {
                    state
                }
            }
            ViewMode::TmuxView(_) => {
                if let Some(tmux) = &state.tmux_view {
                    AppState {
                        tmux_view: Some(tmux.scroll_up(10)),
                        ..state
                    }
                } else {
                    state
                }
            }
            _ => state,
        },

        Action::PageDown => match &state.view {
            ViewMode::TaskDetail(_) => {
                if let Some(detail) = &state.task_detail {
                    AppState {
                        task_detail: Some(detail.scroll_down(10, 100)),
                        ..state
                    }
                } else {
                    state
                }
            }
            ViewMode::TmuxView(_) => {
                if let Some(tmux) = &state.tmux_view {
                    AppState {
                        tmux_view: Some(tmux.scroll_down(10)),
                        ..state
                    }
                } else {
                    state
                }
            }
            _ => state,
        },

        Action::SwitchToList => AppState {
            view: ViewMode::TaskList,
            task_detail: None,
            tmux_view: None,
            ..state
        },

        Action::SwitchToDetail(name) => AppState {
            view: ViewMode::TaskDetail(name),
            ..state
        },

        Action::SwitchToTmux(name) => AppState {
            view: ViewMode::TmuxView(name.clone()),
            tmux_view: Some(TmuxViewState::new(name)),
            ..state
        },

        Action::ShowHelp => AppState {
            modal: Some(ModalState::Help),
            ..state
        },

        Action::HideModal => AppState {
            modal: None,
            ..state
        },

        // Task operations - these are handled by the app loop, reducer just passes through
        Action::StartTask(_)
        | Action::StopTask(_)
        | Action::ResetTask(_)
        | Action::NextTask(_)
        | Action::RetryTask(_)
        | Action::SkipTask(_)
        | Action::DoneTask(_)
        | Action::FailTask(_)
        | Action::BlockTask(_)
        | Action::Refresh
        | Action::Tick => state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskStatus;
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
    fn test_quit() {
        let state = AppState::default();
        let state = reduce(state, Action::Quit);
        assert!(state.should_quit);
    }

    #[test]
    fn test_navigation() {
        let tasks = vec![make_task("task1"), make_task("task2")];
        let state = AppState::with_tasks(tasks);

        assert_eq!(state.task_list.selected, 0);

        let state = reduce(state, Action::SelectNext);
        assert_eq!(state.task_list.selected, 1);

        let state = reduce(state, Action::SelectPrev);
        assert_eq!(state.task_list.selected, 0);
    }

    #[test]
    fn test_view_switching() {
        let tasks = vec![make_task("task1")];
        let state = AppState::with_tasks(tasks);
        assert_eq!(state.view, ViewMode::TaskList);

        // Enter detail
        let state = reduce(state, Action::Enter);
        assert_eq!(state.view, ViewMode::TaskDetail("task1".to_string()));

        // Back to list
        let state = reduce(state, Action::Back);
        assert_eq!(state.view, ViewMode::TaskList);

        // Back at list does nothing
        let state = reduce(state, Action::Back);
        assert_eq!(state.view, ViewMode::TaskList);
    }

    #[test]
    fn test_modal() {
        let state = AppState::default();
        assert!(state.modal.is_none());

        let state = reduce(state, Action::ShowHelp);
        assert_eq!(state.modal, Some(ModalState::Help));

        let state = reduce(state, Action::HideModal);
        assert!(state.modal.is_none());
    }
}
