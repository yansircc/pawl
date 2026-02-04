use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const STATUS_FILE: &str = "status.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusStore {
    /// Task states by task name
    pub tasks: HashMap<String, TaskState>,
}

impl StatusStore {
    /// Load status from .wf/status.json
    pub fn load<P: AsRef<Path>>(wf_dir: P) -> Result<Self> {
        let path = wf_dir.as_ref().join(STATUS_FILE);
        Self::load_from(&path)
    }

    /// Load status from a specific path
    pub fn load_from<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read status: {}", path.display()))?;
        serde_json::from_str(&content).context("Failed to parse status JSON")
    }

    /// Save status to .wf/status.json with atomic write
    pub fn save<P: AsRef<Path>>(&self, wf_dir: P) -> Result<()> {
        let path = wf_dir.as_ref().join(STATUS_FILE);
        self.save_to(&path)
    }

    /// Save status to a specific path with atomic write
    pub fn save_to<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let content = serde_json::to_string_pretty(self).context("Failed to serialize status")?;

        // Atomic write: write to tmp file, then rename
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, &content)
            .with_context(|| format!("Failed to write temp file: {}", tmp_path.display()))?;
        fs::rename(&tmp_path, path)
            .with_context(|| format!("Failed to rename to: {}", path.display()))?;

        Ok(())
    }

    /// Get task state, or None if task not started
    pub fn get(&self, task: &str) -> Option<&TaskState> {
        self.tasks.get(task)
    }

    /// Get mutable task state
    pub fn get_mut(&mut self, task: &str) -> Option<&mut TaskState> {
        self.tasks.get_mut(task)
    }

    /// Insert or update task state
    pub fn set(&mut self, task: String, state: TaskState) {
        self.tasks.insert(task, state);
    }

    /// Remove task state (for reset)
    pub fn remove(&mut self, task: &str) -> Option<TaskState> {
        self.tasks.remove(task)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    /// Current step index (0-based)
    pub current_step: usize,

    /// Overall task status
    pub status: TaskStatus,

    /// When the task was started
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,

    /// When the status was last updated
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,

    /// Status of each step (by step index)
    #[serde(default)]
    pub step_status: HashMap<usize, StepStatus>,

    /// Optional message (e.g., failure reason)
    #[serde(default)]
    pub message: Option<String>,
}

impl TaskState {
    /// Create a new task state in pending status
    pub fn new() -> Self {
        Self {
            current_step: 0,
            status: TaskStatus::Pending,
            started_at: None,
            updated_at: None,
            step_status: HashMap::new(),
            message: None,
        }
    }

    /// Create a new task state and mark as started
    pub fn started() -> Self {
        let now = Utc::now();
        Self {
            current_step: 0,
            status: TaskStatus::Running,
            started_at: Some(now),
            updated_at: Some(now),
            step_status: HashMap::new(),
            message: None,
        }
    }

    /// Update the updated_at timestamp
    pub fn touch(&mut self) {
        self.updated_at = Some(Utc::now());
    }

    /// Mark step as completed with given status
    pub fn mark_step(&mut self, step_idx: usize, status: StepStatus) {
        self.step_status.insert(step_idx, status);
        self.touch();
    }
}

impl Default for TaskState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    #[default]
    Pending,
    Running,
    Waiting,
    Completed,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Success,
    Failed,
    Blocked,
    Skipped,
}
