use anyhow::{bail, Context, Result};
use std::path::PathBuf;

use crate::model::{Config, StatusStore, TaskDefinition};
use crate::util::git::get_repo_root;

const WF_DIR: &str = ".wf";

/// Project context with loaded config and status
pub struct Project {
    pub repo_root: String,
    pub wf_dir: PathBuf,
    pub config: Config,
    pub status: StatusStore,
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
        let status = StatusStore::load(&wf_dir)?;

        Ok(Self {
            repo_root,
            wf_dir,
            config,
            status,
        })
    }

    /// Save status back to disk
    pub fn save_status(&self) -> Result<()> {
        self.status.save(&self.wf_dir)
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
        // Check if it's a number (1-based index)
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

        // It's a task name
        Ok(name_or_index.to_string())
    }

    /// Check if all dependencies of a task are completed
    pub fn check_dependencies(&self, task: &TaskDefinition) -> Result<Vec<String>> {
        let mut blocking = Vec::new();
        for dep in &task.depends {
            match self.status.get(dep) {
                Some(state) if state.status == crate::model::TaskStatus::Completed => {
                    // Dependency satisfied
                }
                _ => {
                    blocking.push(dep.clone());
                }
            }
        }
        Ok(blocking)
    }
}
