use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// tmux session name (default: project directory name)
    #[serde(default)]
    pub session: Option<String>,

    /// Viewport backend (default: "tmux")
    #[serde(default = "default_viewport")]
    pub viewport: String,

    /// User-defined variables (expanded in definition order)
    #[serde(default)]
    pub vars: IndexMap<String, String>,

    /// Per-task metadata (description, depends, skip)
    #[serde(default)]
    pub tasks: IndexMap<String, TaskConfig>,

    /// Workflow steps
    pub workflow: Vec<Step>,

    /// Event hooks: event type (snake_case) -> shell command
    /// Keys match Event enum serde tags: task_started, step_finished, etc.
    #[serde(default)]
    pub on: HashMap<String, String>,
}

fn default_viewport() -> String {
    "tmux".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Step name
    pub name: String,

    /// Command to run (None = gate step)
    #[serde(default)]
    pub run: Option<String>,

    /// Whether to run in a viewport
    #[serde(default)]
    pub in_viewport: bool,

    /// Verifier: a shell command (must exit 0) or "manual" for manual approval
    #[serde(default)]
    pub verify: Option<String>,

    /// Failure strategy: "retry" (auto-retry) or "manual" (wait for decision)
    #[serde(default)]
    pub on_fail: Option<String>,

    /// Max auto-retries when on_fail="retry" (default: 3)
    #[serde(default)]
    pub max_retries: Option<usize>,
}

impl Step {
    /// Gate step: no run command (waits for approval or passes through)
    pub fn is_gate(&self) -> bool {
        self.run.is_none()
    }

    /// Effective max retries (default: 3)
    pub fn effective_max_retries(&self) -> usize {
        self.max_retries.unwrap_or(3)
    }
}

impl Config {
    /// Load config from .pawl/config.json
    pub fn load<P: AsRef<Path>>(pawl_dir: P) -> Result<Self> {
        let config_path = pawl_dir.as_ref().join("config.json");
        Self::load_from(&config_path)
    }

    /// Load config from a specific path
    pub fn load_from<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;
        Self::from_str(&content)
    }

    /// Parse config from JSON string
    pub fn from_str(content: &str) -> Result<Self> {
        let config: Self = serde_json::from_str(content).context("Failed to parse config JSON")?;

        for step in &config.workflow {
            if step.run.is_none() && (step.verify.is_some() || step.on_fail.is_some()) {
                eprintln!(
                    "Warning: step '{}' has verify/on_fail but no run command — it will be treated as a gate step.",
                    step.name
                );
            }
            if step.in_viewport {
                if step.verify.is_none() {
                    eprintln!(
                        "Warning: step '{}' (in_viewport) has no verify — `pawl done` will assume success unconditionally.",
                        step.name
                    );
                }
                if step.verify.is_some() && step.on_fail.is_none() {
                    eprintln!(
                        "Warning: step '{}' (in_viewport) has verify but no on_fail — verify failure is terminal.",
                        step.name
                    );
                }
            }
        }

        Ok(config)
    }

    /// Get session name, defaulting to directory name
    pub fn session_name(&self, project_dir: &str) -> String {
        self.session.clone().unwrap_or_else(|| {
            Path::new(project_dir)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("pawl")
                .to_string()
        })
    }
}
