use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Session name prefix
    #[serde(default)]
    pub session: Option<String>,

    /// Terminal multiplexer to use (tmux, zellij)
    #[serde(default = "default_multiplexer")]
    pub multiplexer: String,

    /// Workflow configuration
    pub workflow: Workflow,

    /// Hooks configuration
    #[serde(default)]
    pub hooks: Hooks,
}

fn default_multiplexer() -> String {
    "tmux".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// Workflow steps
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Step name
    pub name: String,

    /// Command to run
    #[serde(default)]
    pub run: Option<String>,

    /// Whether to run in a new window
    #[serde(default)]
    pub in_window: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Hooks {
    /// Hook to run on task start
    #[serde(default)]
    pub on_start: Option<String>,

    /// Hook to run on task complete
    #[serde(default)]
    pub on_complete: Option<String>,

    /// Hook to run on task fail
    #[serde(default)]
    pub on_fail: Option<String>,
}
