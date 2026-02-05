use anyhow::{bail, Result};
use std::path::PathBuf;

use crate::model::{Config, StatusStore, TaskDefinition, TaskStatus};
use crate::tui::state::task_detail::{StepItem, StepItemStatus, StepType};
use crate::tui::state::{TaskDetailState, TaskItem};
use crate::util::{git, tmux};

use super::provider::{DataProvider, TaskAction, TmuxCaptureResult};

/// Live data provider that reads from actual files and tmux
pub struct LiveDataProvider {
    repo_root: String,
    wf_dir: PathBuf,
}

impl LiveDataProvider {
    pub fn new() -> Result<Self> {
        let repo_root = git::get_repo_root()?;
        let wf_dir = PathBuf::from(&repo_root).join(".wf");

        if !wf_dir.exists() {
            bail!("Not a wf project. Run 'wf init' first.");
        }

        Ok(Self { repo_root, wf_dir })
    }

    fn load_config(&self) -> Result<Config> {
        Config::load(&self.wf_dir)
    }

    fn load_status(&self) -> Result<StatusStore> {
        StatusStore::load(&self.wf_dir)
    }

    fn session_name(&self) -> Result<String> {
        let config = self.load_config()?;
        Ok(config.session_name(&self.repo_root))
    }

    fn check_dependencies(&self, task: &TaskDefinition, status: &StatusStore) -> Vec<String> {
        let mut blocking = Vec::new();
        for dep in &task.depends {
            match status.get(dep) {
                Some(state) if state.status == TaskStatus::Completed => {}
                _ => blocking.push(dep.clone()),
            }
        }
        blocking
    }
}

impl DataProvider for LiveDataProvider {
    fn load_tasks(&self) -> Result<Vec<TaskItem>> {
        let config = self.load_config()?;
        let status = self.load_status()?;
        let tasks = TaskDefinition::load_all(self.wf_dir.join("tasks"))?;
        let workflow_len = config.workflow.len();

        let items: Vec<TaskItem> = tasks
            .iter()
            .map(|task_def| {
                let blocked_by = self.check_dependencies(task_def, &status);

                if let Some(state) = status.get(&task_def.name) {
                    let step_name = if state.current_step < workflow_len {
                        config.workflow[state.current_step].name.clone()
                    } else {
                        "Done".to_string()
                    };

                    TaskItem {
                        name: task_def.name.clone(),
                        status: state.status,
                        current_step: state.current_step,
                        total_steps: workflow_len,
                        step_name,
                        blocked_by,
                        message: state.message.clone(),
                    }
                } else {
                    TaskItem {
                        name: task_def.name.clone(),
                        status: TaskStatus::Pending,
                        current_step: 0,
                        total_steps: workflow_len,
                        step_name: "--".to_string(),
                        blocked_by,
                        message: None,
                    }
                }
            })
            .collect();

        Ok(items)
    }

    fn load_task_detail(&self, name: &str) -> Result<TaskDetailState> {
        let config = self.load_config()?;
        let status = self.load_status()?;
        let task_def = TaskDefinition::load(self.wf_dir.join("tasks").join(format!("{}.md", name)))?;

        let state = status.get(name);
        let current_step = state.map(|s| s.current_step).unwrap_or(0);
        let task_status = state.map(|s| s.status).unwrap_or(TaskStatus::Pending);

        let steps: Vec<StepItem> = config
            .workflow
            .iter()
            .enumerate()
            .map(|(i, step)| {
                let step_type = if step.is_checkpoint() {
                    StepType::Checkpoint
                } else if step.in_window {
                    StepType::InWindow
                } else {
                    StepType::Normal
                };

                let status = if let Some(state) = state {
                    if i < current_step {
                        state
                            .step_status
                            .get(&i)
                            .map(|s| StepItemStatus::from(*s))
                            .unwrap_or(StepItemStatus::Success)
                    } else if i == current_step {
                        StepItemStatus::Current
                    } else {
                        StepItemStatus::Pending
                    }
                } else {
                    StepItemStatus::Pending
                };

                StepItem {
                    index: i,
                    name: step.name.clone(),
                    step_type,
                    status,
                }
            })
            .collect();

        Ok(TaskDetailState::new(
            name.to_string(),
            task_def.description,
            task_def.depends,
            task_status,
            current_step,
            steps,
            state.and_then(|s| s.message.clone()),
        ))
    }

    fn capture_tmux(&self, task_name: &str, lines: usize) -> Result<TmuxCaptureResult> {
        let session = self.session_name()?;

        match tmux::capture_pane(&session, task_name, lines)? {
            tmux::CaptureResult::Content(content) => Ok(TmuxCaptureResult {
                content,
                window_exists: true,
            }),
            tmux::CaptureResult::WindowGone => Ok(TmuxCaptureResult {
                content: String::new(),
                window_exists: false,
            }),
        }
    }

    fn execute_action(&self, action: &TaskAction) -> Result<()> {
        use crate::cmd::{agent, control, start};

        match action {
            TaskAction::Start(name) => start::run(name),
            TaskAction::Stop(name) => control::stop(name),
            TaskAction::Reset(name) => control::reset(name),
            TaskAction::Next(name) => control::next(name),
            TaskAction::Retry(name) => control::retry(name),
            TaskAction::Skip(name) => control::skip(name),
            TaskAction::Done(name) => agent::done(name, None),
            TaskAction::Fail(name) => agent::fail(name, None),
            TaskAction::Block(name) => agent::block(name, None),
        }
    }
}
