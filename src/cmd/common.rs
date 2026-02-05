use anyhow::{bail, Result};
use std::path::PathBuf;

use crate::model::{Config, StatusStore, TaskDefinition};
use crate::util::git::get_repo_root;
use crate::util::shell::spawn_background;
use crate::util::variable::Context;

/// Convert a step name to a safe filename slug
pub fn slugify(name: &str) -> String {
    let mut result = String::new();
    let mut last_was_dash = true; // Start true to avoid leading dash

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            result.push('-');
            last_was_dash = true;
        }
    }

    // Remove trailing dash
    if result.ends_with('-') {
        result.pop();
    }

    result
}

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

    /// Get the log directory for a task
    pub fn log_dir(&self, task_name: &str) -> PathBuf {
        self.wf_dir.join("logs").join(task_name)
    }

    /// Get the log file path for a specific step
    pub fn log_path(&self, task_name: &str, step_idx: usize, step_name: &str) -> PathBuf {
        let slug = slugify(step_name);
        let filename = format!("step-{}-{}.log", step_idx + 1, slug);
        self.log_dir(task_name).join(filename)
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
            );
            let expanded = ctx.expand(cmd);
            if let Err(e) = spawn_background(&expanded) {
                eprintln!("Warning: hook '{}' failed: {}", event, e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_simple() {
        assert_eq!(slugify("hello"), "hello");
        assert_eq!(slugify("Hello World"), "hello-world");
    }

    #[test]
    fn test_slugify_special_chars() {
        assert_eq!(slugify("Setup: Environment"), "setup-environment");
        assert_eq!(slugify("Run [tests]"), "run-tests");
        assert_eq!(slugify("Step #1 - Build"), "step-1-build");
    }

    #[test]
    fn test_slugify_consecutive_non_alnum() {
        assert_eq!(slugify("a---b"), "a-b");
        assert_eq!(slugify("a   b"), "a-b");
        assert_eq!(slugify("a!@#b"), "a-b");
    }

    #[test]
    fn test_slugify_leading_trailing() {
        assert_eq!(slugify("--hello--"), "hello");
        assert_eq!(slugify("  hello  "), "hello");
        assert_eq!(slugify("...test..."), "test");
    }

    #[test]
    fn test_slugify_empty() {
        assert_eq!(slugify(""), "");
        assert_eq!(slugify("---"), "");
    }
}
