use anyhow::Result;
use fs2::FileExt;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use crate::error::PawlError;
use crate::model::config::TaskConfig;
use crate::model::event::{event_timestamp, replay, Event};
use crate::model::{Config, TaskState, TaskStatus};
use crate::util::project::get_project_root;
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
                    if let Some(vo) = verify_output
                        && !vo.is_empty() { parts.push(vo.as_str()); }
                    if let Some(out) = stdout
                        && !out.is_empty() { parts.push(out.as_str()); }
                    if let Some(err) = stderr
                        && !err.is_empty() { parts.push(err.as_str()); }
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

/// Project context with loaded workflows
pub struct Project {
    pub project_root: String,
    pub pawl_dir: PathBuf,
    workflows: IndexMap<String, Config>,
    task_index: HashMap<String, String>,
}

impl Project {
    /// Load project from current directory — scans .pawl/workflows/*.json
    pub fn load() -> Result<Self> {
        let project_root = get_project_root()?;
        let pawl_dir = PathBuf::from(&project_root).join(PAWL_DIR);
        let workflows_dir = pawl_dir.join("workflows");

        if !workflows_dir.exists() {
            return Err(PawlError::NotFound {
                message: "No .pawl/workflows/ directory found. Run 'pawl init' first.".into(),
            }.into());
        }

        let mut workflows = IndexMap::new();
        let mut task_index = HashMap::new();

        let mut entries: Vec<_> = fs::read_dir(&workflows_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            let wf_name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if wf_name.is_empty() {
                continue;
            }

            let config = Config::load_from(&path)?;

            // Validate task name uniqueness across workflows
            for task_name in config.tasks.keys() {
                if let Some(existing_wf) = task_index.get(task_name) {
                    return Err(PawlError::Validation {
                        message: format!(
                            "Task '{}' declared in both '{}' and '{}' workflows. Task names must be globally unique.",
                            task_name, existing_wf, wf_name
                        ),
                    }.into());
                }
                task_index.insert(task_name.clone(), wf_name.clone());
            }

            workflows.insert(wf_name, config);
        }

        if workflows.is_empty() {
            return Err(PawlError::NotFound {
                message: "No workflow files found in .pawl/workflows/. Run 'pawl init' first.".into(),
            }.into());
        }

        Ok(Self {
            project_root,
            pawl_dir,
            workflows,
            task_index,
        })
    }

    /// Find the workflow name and config for a given task.
    /// Falls back to the first workflow if the task is undeclared (e.g. ad-hoc tasks).
    pub fn workflow_for(&self, task_name: &str) -> Result<(&str, &Config)> {
        if let Some(wf_name) = self.task_index.get(task_name) {
            let config = self.workflows.get(wf_name).unwrap();
            return Ok((wf_name, config));
        }
        // Undeclared task: use the first workflow (backward-compatible with single-workflow projects)
        if self.workflows.len() == 1 {
            let (name, config) = self.workflows.first().unwrap();
            return Ok((name, config));
        }
        Err(PawlError::NotFound {
            message: format!(
                "Task '{}' not declared in any workflow. With multiple workflows, all tasks must be declared.",
                task_name
            ),
        }.into())
    }

    /// Create a viewport for the given task's workflow
    pub fn viewport_for(&self, task_name: &str) -> Result<Box<dyn Viewport>> {
        let (_, config) = self.workflow_for(task_name)?;
        let session = self.session_name_for(task_name)?;
        viewport::create_viewport(&config.viewport, &session)
    }

    /// Get all workflows
    pub fn all_workflows(&self) -> &IndexMap<String, Config> {
        &self.workflows
    }

    /// Get session name for a task's workflow
    pub fn session_name_for(&self, task_name: &str) -> Result<String> {
        let (_, config) = self.workflow_for(task_name)?;
        Ok(config.session_name(&self.project_root))
    }

    /// Build a Context for variable expansion / env vars.
    /// Intrinsic vars first, then user vars from the task's workflow config.vars (expanded in order).
    pub fn context_for(&self, task_name: &str, step_idx: Option<usize>, run_id: &str) -> Context {
        let (wf_name, config) = self.workflow_for(task_name).unwrap_or_else(|_| {
            let (name, config) = self.workflows.first().unwrap();
            (name, config)
        });

        let step_name = step_idx
            .and_then(|i| config.workflow.get(i))
            .map(|s| s.name.as_str())
            .unwrap_or("");

        let session = config.session_name(&self.project_root);

        let mut ctx = Context::build()
            .var("task", task_name)
            .var("workflow", wf_name)
            .var("session", session)
            .var("project_root", &self.project_root)
            .var("step", step_name)
            .var("step_index", step_idx.map(|i| i.to_string()).unwrap_or_default())
            .var("log_file", self.log_file(task_name).to_string_lossy())
            .var("run_id", run_id);

        // Expand user vars in definition order (earlier vars available to later)
        for (key, value) in &config.vars {
            let expanded = ctx.expand(value);
            ctx = ctx.var_owned(key.clone(), expanded);
        }

        ctx
    }

    /// Get step name by index for a task, returns "done" if past end.
    pub fn step_name(&self, task_name: &str, step_idx: usize) -> &str {
        if let Ok((_, config)) = self.workflow_for(task_name) {
            config.workflow.get(step_idx)
                .map(|s| s.name.as_str())
                .unwrap_or("done")
        } else {
            "done"
        }
    }

    /// Get task config from the task's workflow (returns None for undeclared tasks)
    pub fn task_config(&self, name: &str) -> Option<&TaskConfig> {
        if let Some(wf_name) = self.task_index.get(name) {
            self.workflows.get(wf_name).and_then(|c| c.tasks.get(name))
        } else {
            // Undeclared task: check all workflows
            for config in self.workflows.values() {
                if let Some(tc) = config.tasks.get(name) {
                    return Some(tc);
                }
            }
            None
        }
    }

    /// Discover all known tasks: all workflow tasks keys ∪ log file stems, sorted
    pub fn discover_tasks(&self) -> Result<Vec<String>> {
        let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for config in self.workflows.values() {
            names.extend(config.tasks.keys().cloned());
        }
        let logs_dir = self.pawl_dir.join("logs");
        if logs_dir.exists() {
            for entry in fs::read_dir(&logs_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map(|e| e == "jsonl").unwrap_or(false)
                    && let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        names.insert(stem.to_string());
                    }
            }
        }
        Ok(names.into_iter().collect())
    }

    /// Resolve task name from name or 1-based index
    pub fn resolve_task_name(&self, name_or_index: &str) -> Result<String> {
        if let Ok(index) = name_or_index.parse::<usize>() {
            let tasks = self.discover_tasks()?;
            if index == 0 || index > tasks.len() {
                return Err(PawlError::NotFound {
                    message: format!("Task index {} out of range. Have {} tasks.", index, tasks.len()),
                }.into());
            }
            return Ok(tasks[index - 1].clone());
        }
        Ok(name_or_index.to_string())
    }

    /// Check if all dependencies of a task are completed
    pub fn check_dependencies(&self, task_name: &str) -> Result<Vec<String>> {
        let depends = match self.task_config(task_name) {
            Some(tc) => &tc.depends,
            None => return Ok(Vec::new()),
        };
        let mut blocking = Vec::new();
        for dep in depends {
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

    /// Get the stream file path for a task (live stdout during execution)
    pub fn stream_file(&self, task_name: &str) -> PathBuf {
        self.pawl_dir.join("streams").join(format!("{}.stream", task_name))
    }

    /// Append an event to the task's JSONL log file (with exclusive file lock),
    /// then auto-fire any matching hook from the task's workflow config.on.
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
        let workflow_len = self.workflow_for(task_name)
            .map(|(_, c)| c.workflow.len())
            .unwrap_or(0);
        Ok(replay(&events, workflow_len))
    }

    /// Check viewport health. If a Running in_viewport step's viewport is gone, emit ViewportLost.
    /// Returns true = healthy (or not applicable), false = ViewportLost emitted.
    pub fn detect_viewport_loss(&self, task_name: &str) -> Result<bool> {
        let state = self.replay_task(task_name)?;

        if let Some(ref s) = state
            && s.status == TaskStatus::Running {
                let step_idx = s.current_step;
                if let Ok((_, config)) = self.workflow_for(task_name) {
                    if step_idx < config.workflow.len()
                        && config.workflow[step_idx].in_viewport
                    {
                        if let Ok(vp) = self.viewport_for(task_name) {
                            if !vp.exists(task_name) {
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
                }
            }

        Ok(true)
    }

    /// Fire a hook for an event (fire-and-forget).
    /// Looks up the task's workflow config.on by the event's serde tag name.
    fn spawn_event_hook(&self, task_name: &str, event: &Event) {
        let event_type = event.type_name();
        let config = match self.workflow_for(task_name) {
            Ok((_, c)) => c,
            Err(_) => return,
        };
        let Some(cmd) = config.on.get(event_type) else {
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

    /// Output task state as JSON to stdout — unified output point for all write commands.
    pub fn output_task_state(&self, task_name: &str) -> Result<()> {
        self.detect_viewport_loss(task_name)?;
        let state = self.replay_task(task_name)?;
        let events = self.read_events(task_name)?;
        let (wf_name, config) = self.workflow_for(task_name)?;
        let workflow_len = config.workflow.len();

        let (current_step, status, run_id, message) = if let Some(s) = &state {
            (s.current_step, s.status.to_string(), s.run_id.clone(), s.message.clone())
        } else {
            (0, "pending".to_string(), String::new(), None)
        };

        let (retry_count, last_feedback) = extract_step_context(&events, current_step);

        let json = serde_json::json!({
            "name": task_name,
            "workflow": wf_name,
            "status": status,
            "run_id": run_id,
            "current_step": current_step,
            "step_name": self.step_name(task_name, current_step),
            "total_steps": workflow_len,
            "message": message,
            "retry_count": retry_count,
            "last_feedback": last_feedback,
        });
        println!("{}", json);
        Ok(())
    }
}
