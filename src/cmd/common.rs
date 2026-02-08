use anyhow::Result;
use fs2::FileExt;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use crate::error::PawlError;
use crate::model::event::{event_timestamp, replay, Event};
use crate::model::{Config, TaskDefinition, TaskState, TaskStatus};
use crate::util::git::get_repo_root;
use crate::util::shell::spawn_background;
use crate::util::variable::Context;
use crate::viewport::{self, Viewport};

/// Extract retry_count and last_feedback for the current step from events.
pub fn extract_step_context(events: &[Event], step_idx: usize) -> (usize, Option<String>) {
    let retry_count = crate::model::event::count_auto_retries(events, step_idx);
    let mut last_feedback: Option<String> = None;

    for event in events.iter().rev() {
        match event {
            Event::TaskStarted { .. } | Event::TaskReset { .. } => break,
            Event::StepReset { step, auto: false, .. } if *step == step_idx => break,
            Event::StepFinished { step, success, stdout, stderr, verify_output, .. }
                if *step == step_idx && !*success =>
            {
                if last_feedback.is_none() {
                    let mut parts = Vec::new();
                    if let Some(vo) = verify_output {
                        if !vo.is_empty() { parts.push(vo.as_str()); }
                    }
                    if let Some(out) = stdout {
                        if !out.is_empty() { parts.push(out.as_str()); }
                    }
                    if let Some(err) = stderr {
                        if !err.is_empty() { parts.push(err.as_str()); }
                    }
                    if !parts.is_empty() {
                        last_feedback = Some(parts.join("\n"));
                    }
                }
            }
            _ => {}
        }
    }

    (retry_count, last_feedback)
}

pub const PAWL_DIR: &str = ".pawl";

/// Project context with loaded config
pub struct Project {
    pub repo_root: String,
    pub pawl_dir: PathBuf,
    pub config: Config,
    pub viewport: Box<dyn Viewport>,
}

impl Project {
    /// Load project from current directory
    pub fn load() -> Result<Self> {
        let repo_root = get_repo_root()?;
        let pawl_dir = PathBuf::from(&repo_root).join(PAWL_DIR);

        if !pawl_dir.exists() {
            return Err(PawlError::NotFound {
                message: "Not a pawl project. Run 'pawl init' first.".into(),
            }.into());
        }

        let config = Config::load(&pawl_dir)?;
        let session = config.session_name(&repo_root);
        let vp = viewport::create_viewport(&config.viewport, &session)?;

        Ok(Self {
            repo_root,
            pawl_dir,
            config,
            viewport: vp,
        })
    }

    /// Build a Context for variable expansion / env vars.
    pub fn context_for(&self, task_name: &str, step_idx: Option<usize>, run_id: &str) -> Context {
        let step_name = step_idx
            .and_then(|i| self.config.workflow.get(i))
            .map(|s| s.name.as_str())
            .unwrap_or("");

        Context::build()
            .var("task", task_name)
            .var("branch", format!("pawl/{}", task_name))
            .var("worktree", self.worktree_path(task_name).to_string_lossy())
            .var("session", self.session_name())
            .var("repo_root", &self.repo_root)
            .var("step", step_name)
            .var("base_branch", &self.config.base_branch)
            .var("step_index", step_idx.map(|i| i.to_string()).unwrap_or_default())
            .var("log_file", self.log_file(task_name).to_string_lossy())
            .var("task_file", self.task_file(task_name).to_string_lossy())
            .var("run_id", run_id)
    }

    /// Get step name by index, returns "done" if past end.
    pub fn step_name(&self, step_idx: usize) -> &str {
        self.config.workflow.get(step_idx)
            .map(|s| s.name.as_str())
            .unwrap_or("done")
    }

    /// Get the worktree path for a task.
    pub fn worktree_path(&self, task_name: &str) -> PathBuf {
        PathBuf::from(&self.repo_root).join(&self.config.worktree_dir).join(task_name)
    }

    /// Get session name
    pub fn session_name(&self) -> String {
        self.config.session_name(&self.repo_root)
    }

    /// Load a task definition by name
    pub fn load_task(&self, name: &str) -> Result<TaskDefinition> {
        let task_path = self.pawl_dir.join("tasks").join(format!("{}.md", name));
        if !task_path.exists() {
            return Err(PawlError::NotFound {
                message: format!("Task '{}' not found. Create it with: pawl create {}", name, name),
            }.into());
        }
        TaskDefinition::load(&task_path)
    }

    /// Load all task definitions
    pub fn load_all_tasks(&self) -> Result<Vec<TaskDefinition>> {
        TaskDefinition::load_all(self.pawl_dir.join("tasks"))
    }

    /// Resolve task name from name or 1-based index
    pub fn resolve_task_name(&self, name_or_index: &str) -> Result<String> {
        if let Ok(index) = name_or_index.parse::<usize>() {
            let tasks = self.load_all_tasks()?;
            if index == 0 || index > tasks.len() {
                return Err(PawlError::NotFound {
                    message: format!("Task index {} out of range. Have {} tasks.", index, tasks.len()),
                }.into());
            }
            return Ok(tasks[index - 1].name.clone());
        }
        Ok(name_or_index.to_string())
    }

    /// Check if all dependencies of a task are completed
    pub fn check_dependencies(&self, task: &TaskDefinition) -> Result<Vec<String>> {
        let mut blocking = Vec::new();
        for dep in &task.depends {
            let state = self.replay_task(dep)?;
            match state {
                Some(s) if s.status == TaskStatus::Completed => {}
                _ => blocking.push(dep.clone()),
            }
        }
        Ok(blocking)
    }

    /// Get the JSONL log file path for a task
    pub fn log_file(&self, task_name: &str) -> PathBuf {
        self.pawl_dir.join("logs").join(format!("{}.jsonl", task_name))
    }

    /// Get the task definition file path
    pub fn task_file(&self, task_name: &str) -> PathBuf {
        self.pawl_dir.join("tasks").join(format!("{}.md", task_name))
    }

    /// Append an event to the task's JSONL log file (with exclusive file lock),
    /// then auto-fire any matching hook from config.on.
    pub fn append_event(&self, task_name: &str, event: &Event) -> Result<()> {
        let log_file = self.log_file(task_name);
        let log_dir = log_file.parent().unwrap();

        fs::create_dir_all(log_dir)?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)?;

        file.lock_exclusive()?;

        let json = serde_json::to_string(event)?;
        writeln!(file, "{}", json)?;

        file.unlock()?;

        // Auto-fire hook if configured
        self.spawn_event_hook(task_name, event);

        Ok(())
    }

    /// Read all events from the task's JSONL log file
    pub fn read_events(&self, task_name: &str) -> Result<Vec<Event>> {
        let log_file = self.log_file(task_name);

        if !log_file.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&log_file)?;
        file.lock_shared()?;
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

        Ok(events)
    }

    /// Replay events to reconstruct current TaskState
    pub fn replay_task(&self, task_name: &str) -> Result<Option<TaskState>> {
        let events = self.read_events(task_name)?;
        let workflow_len = self.config.workflow.len();
        Ok(replay(&events, workflow_len))
    }

    /// Check viewport health. If a Running in_viewport step's viewport is gone, emit ViewportLost.
    /// Returns true = healthy (or not applicable), false = ViewportLost emitted.
    pub fn detect_viewport_loss(&self, task_name: &str) -> Result<bool> {
        let state = self.replay_task(task_name)?;

        if let Some(ref s) = state {
            if s.status == TaskStatus::Running {
                let step_idx = s.current_step;
                if step_idx < self.config.workflow.len()
                    && self.config.workflow[step_idx].in_viewport
                    && !self.viewport.exists(task_name)
                {
                    self.append_event(
                        task_name,
                        &Event::ViewportLost {
                            ts: event_timestamp(),
                            step: step_idx,
                        },
                    )?;
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Fire a hook for an event (fire-and-forget).
    /// Looks up config.on by the event's serde tag name, expands variables, spawns in background.
    fn spawn_event_hook(&self, task_name: &str, event: &Event) {
        let event_type = event.type_name();
        let Some(cmd) = self.config.on.get(event_type) else {
            return;
        };

        let step_idx = event.step_index();
        let run_id = self.replay_task(task_name)
            .ok()
            .flatten()
            .map(|s| s.run_id)
            .unwrap_or_default();
        let mut ctx = self.context_for(task_name, step_idx, &run_id);

        // Inject retry context variables
        if let Some(si) = step_idx {
            let events = self.read_events(task_name).unwrap_or_default();
            let (retry_count, last_feedback) = extract_step_context(&events, si);
            ctx = ctx.var("retry_count", retry_count.to_string());
            if let Some(fb) = &last_feedback {
                ctx = ctx.var("last_verify_output", fb);
            }
        }

        // Extend with event-specific variables (${exit_code}, ${duration}, etc.)
        ctx.extend(event.extra_vars());

        let expanded = ctx.expand(cmd);

        if let Err(e) = spawn_background(&expanded) {
            eprintln!("Warning: hook '{}' failed: {}", event_type, e);
        }
    }

    /// Derive routing hints (suggest/prompt) from task status.
    /// suggest = mechanical commands agent can execute directly.
    /// prompt = requires judgment, agent must evaluate before deciding.
    /// `pawl done` never appears in suggest — it requires judgment.
    pub fn derive_routing(status: &str, message: Option<&str>, task: &str) -> (Vec<String>, Option<String>) {
        match status {
            "pending" => (vec![format!("pawl start {task}")], None),
            "waiting" => match message {
                Some("gate") => (
                    vec![],
                    Some(format!("confirm preconditions, then: pawl done {task}")),
                ),
                Some("verify_human") => (
                    vec![],
                    Some(format!("verify work quality, then: pawl done {task}")),
                ),
                Some("on_fail_human") => (
                    vec![format!("pawl reset --step {task}")],
                    Some(format!("review failure, then: pawl done {task} to accept")),
                ),
                _ => (vec![], None),
            },
            "failed" => (vec![format!("pawl reset --step {task}")], None),
            "stopped" => (
                vec![format!("pawl start {task}"), format!("pawl reset {task}")],
                None,
            ),
            _ => (vec![], None),
        }
    }

    /// Output task state as JSON to stdout — unified output point for all write commands.
    pub fn output_task_state(&self, task_name: &str) -> Result<()> {
        self.detect_viewport_loss(task_name)?;
        let state = self.replay_task(task_name)?;
        let events = self.read_events(task_name)?;
        let workflow_len = self.config.workflow.len();

        let (current_step, status, run_id, message) = if let Some(s) = &state {
            (s.current_step, s.status.to_string(), s.run_id.clone(), s.message.clone())
        } else {
            (0, "pending".to_string(), String::new(), None)
        };

        let (retry_count, last_feedback) = extract_step_context(&events, current_step);
        let (suggest, prompt) = Self::derive_routing(&status, message.as_deref(), task_name);

        let mut json = serde_json::json!({
            "name": task_name,
            "status": status,
            "run_id": run_id,
            "current_step": current_step,
            "step_name": self.step_name(current_step),
            "total_steps": workflow_len,
            "message": message,
            "retry_count": retry_count,
            "last_feedback": last_feedback,
        });
        if !suggest.is_empty() {
            json["suggest"] = serde_json::to_value(&suggest).unwrap();
        }
        if let Some(p) = &prompt {
            json["prompt"] = serde_json::to_value(p).unwrap();
        }
        println!("{}", json);
        Ok(())
    }
}
