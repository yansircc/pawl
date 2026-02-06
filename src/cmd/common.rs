use anyhow::{bail, Result};
use fs2::FileExt;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use crate::model::event::{replay, Event};
use crate::model::{Config, TaskDefinition, TaskState, TaskStatus};
use crate::util::git::get_repo_root;
use crate::util::shell::spawn_background;
use crate::util::variable::Context;

const WF_DIR: &str = ".wf";

/// Project context with loaded config
pub struct Project {
    pub repo_root: String,
    pub wf_dir: PathBuf,
    pub config: Config,
}

impl Project {
    /// Load project from current directory
    pub fn load() -> Result<Self> {
        let repo_root = get_repo_root()?;
        let wf_dir = PathBuf::from(&repo_root).join(WF_DIR);

        if !wf_dir.exists() {
            bail!("Not a wf project. Run 'wf init' first.");
        }

        let config = Config::load(&wf_dir)?;

        Ok(Self {
            repo_root,
            wf_dir,
            config,
        })
    }

    /// Get session name
    pub fn session_name(&self) -> String {
        self.config.session_name(&self.repo_root)
    }

    /// Load a task definition by name
    pub fn load_task(&self, name: &str) -> Result<TaskDefinition> {
        let task_path = self.wf_dir.join("tasks").join(format!("{}.md", name));
        if !task_path.exists() {
            bail!("Task '{}' not found. Create it with: wf create {}", name, name);
        }
        TaskDefinition::load(&task_path)
    }

    /// Load all task definitions
    pub fn load_all_tasks(&self) -> Result<Vec<TaskDefinition>> {
        TaskDefinition::load_all(self.wf_dir.join("tasks"))
    }

    /// Resolve task name from name or 1-based index
    pub fn resolve_task_name(&self, name_or_index: &str) -> Result<String> {
        if let Ok(index) = name_or_index.parse::<usize>() {
            let tasks = self.load_all_tasks()?;
            if index == 0 || index > tasks.len() {
                bail!(
                    "Task index {} out of range. Have {} tasks.",
                    index,
                    tasks.len()
                );
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
        self.wf_dir.join("logs").join(format!("{}.jsonl", task_name))
    }

    /// Get the task definition file path
    pub fn task_file(&self, task_name: &str) -> PathBuf {
        self.wf_dir.join("tasks").join(format!("{}.md", task_name))
    }

    /// Append an event to the task's JSONL log file (with exclusive file lock)
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

        Ok(())
    }

    /// Read all events from the task's JSONL log file
    pub fn read_events(&self, task_name: &str) -> Result<Vec<Event>> {
        let log_file = self.log_file(task_name);

        if !log_file.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&log_file)?;
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

    /// Fire a hook (fire-and-forget)
    pub fn fire_hook(&self, event: &str, task_name: &str) {
        if let Some(cmd) = self.config.hooks.get(event) {
            let ctx = Context::new(
                task_name,
                &self.session_name(),
                &self.repo_root,
                &self.config.worktree_dir,
                event,
                &self.config.base_branch,
            );
            let expanded = ctx.expand(cmd);
            if let Err(e) = spawn_background(&expanded) {
                eprintln!("Warning: hook '{}' failed: {}", event, e);
            }
        }
    }
}
