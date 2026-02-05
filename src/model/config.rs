use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Read as _;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// tmux session name (default: project directory name)
    #[serde(default)]
    pub session: Option<String>,

    /// Terminal multiplexer (default: "tmux")
    #[serde(default = "default_multiplexer")]
    pub multiplexer: String,

    /// Claude CLI command path (default: "claude")
    #[serde(default = "default_claude_command")]
    pub claude_command: String,

    /// Worktree directory relative to repo root (default: ".wf/worktrees")
    #[serde(default = "default_worktree_dir")]
    pub worktree_dir: String,

    /// Workflow steps
    pub workflow: Vec<Step>,

    /// Event hooks: event name -> shell command
    #[serde(default)]
    pub hooks: HashMap<String, String>,
}

fn default_multiplexer() -> String {
    "tmux".to_string()
}

fn default_claude_command() -> String {
    "claude".to_string()
}

fn default_worktree_dir() -> String {
    ".wf/worktrees".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Step name
    pub name: String,

    /// Command to run (None = checkpoint)
    #[serde(default)]
    pub run: Option<String>,

    /// Whether to run in a tmux window
    #[serde(default)]
    pub in_window: bool,

    /// Validation command to run before accepting "wf done"
    /// If specified, must exit with code 0 for done to succeed
    #[serde(default)]
    pub stop_hook: Option<String>,
}

impl Step {
    /// Check if this step is a checkpoint (no run command)
    pub fn is_checkpoint(&self) -> bool {
        self.run.is_none()
    }
}

impl Config {
    /// Load config from .wf/config.jsonc
    pub fn load<P: AsRef<Path>>(wf_dir: P) -> Result<Self> {
        let config_path = wf_dir.as_ref().join("config.jsonc");
        Self::load_from(&config_path)
    }

    /// Load config from a specific path
    pub fn load_from<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;
        Self::from_str(&content)
    }

    /// Parse config from JSONC string
    pub fn from_str(content: &str) -> Result<Self> {
        // Strip comments using json_comments crate
        let mut stripped = String::new();
        json_comments::StripComments::new(content.as_bytes())
            .read_to_string(&mut stripped)
            .context("Failed to strip comments from JSONC")?;

        serde_json::from_str(&stripped).context("Failed to parse config JSON")
    }

    /// Get session name, defaulting to directory name
    pub fn session_name(&self, project_dir: &str) -> String {
        self.session.clone().unwrap_or_else(|| {
            Path::new(project_dir)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("wf")
                .to_string()
        })
    }
}
