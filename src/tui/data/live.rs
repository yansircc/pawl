use anyhow::{bail, Result};
use fs2::FileExt;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use crate::model::event::{event_timestamp, replay, Event};
use crate::model::{Config, TaskDefinition, TaskStatus};
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

    fn session_name(&self) -> Result<String> {
        let config = self.load_config()?;
        Ok(config.session_name(&self.repo_root))
    }

    fn replay_task(&self, task_name: &str, workflow_len: usize) -> Result<Option<crate::model::TaskState>> {
        let log_file = self.wf_dir.join("logs").join(format!("{}.jsonl", task_name));
        if !log_file.exists() {
            return Ok(None);
        }

        let file = std::fs::File::open(&log_file)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: Event = serde_json::from_str(&line)?;
            events.push(event);
        }

        Ok(replay(&events, workflow_len))
    }

    fn append_event(&self, task_name: &str, event: &Event) -> Result<()> {
        let log_file = self.wf_dir.join("logs").join(format!("{}.jsonl", task_name));
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)?;
        file.lock_exclusive()?;
        let json = serde_json::to_string(event)?;
        writeln!(file, "{}", json)?;
        file.unlock()?;
        Ok(())
    }

    /// If task is Running on an in_window step but window is gone, append WindowLost and re-replay.
    fn try_heal_window_lost(
        &self,
        task_name: &str,
        state: &crate::model::TaskState,
        config: &Config,
    ) -> Option<crate::model::TaskState> {
        let step_idx = state.current_step;
        if step_idx >= config.workflow.len() || !config.workflow[step_idx].in_window {
            return None;
        }
        let session = self.session_name().ok()?;
        if tmux::window_exists(&session, task_name) {
            return None;
        }
        let event = Event::WindowLost {
            ts: event_timestamp(),
            step: step_idx,
        };
        self.append_event(task_name, &event).ok()?;
        self.replay_task(task_name, config.workflow.len()).ok()?
    }

    fn check_dependencies(&self, task: &TaskDefinition, workflow_len: usize) -> Vec<String> {
        let mut blocking = Vec::new();
        for dep in &task.depends {
            match self.replay_task(dep, workflow_len) {
                Ok(Some(state)) if state.status == TaskStatus::Completed => {}
                _ => blocking.push(dep.clone()),
            }
        }
        blocking
    }
}

impl DataProvider for LiveDataProvider {
    fn load_tasks(&self) -> Result<Vec<TaskItem>> {
        let config = self.load_config()?;
        let tasks = TaskDefinition::load_all(self.wf_dir.join("tasks"))?;
        let workflow_len = config.workflow.len();

        let items: Vec<TaskItem> = tasks
            .iter()
            .map(|task_def| {
                let blocked_by = self.check_dependencies(task_def, workflow_len);

                if let Ok(Some(state)) = self.replay_task(&task_def.name, workflow_len) {
                    let state = if state.status == TaskStatus::Running {
                        self.try_heal_window_lost(&task_def.name, &state, &config)
                            .unwrap_or(state)
                    } else {
                        state
                    };

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
        let task_def = TaskDefinition::load(self.wf_dir.join("tasks").join(format!("{}.md", name)))?;
        let workflow_len = config.workflow.len();

        let state = self.replay_task(name, workflow_len)?;
        let state = match state {
            Some(s) if s.status == TaskStatus::Running => {
                Some(self.try_heal_window_lost(name, &s, &config).unwrap_or(s))
            }
            other => other,
        };
        let current_step = state.as_ref().map(|s| s.current_step).unwrap_or(0);
        let task_status = state.as_ref().map(|s| s.status).unwrap_or(TaskStatus::Pending);

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

                let status = if let Some(state) = &state {
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
            state.as_ref().and_then(|s| s.message.clone()),
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
