use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "wf", about = "Workflow task manager", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new workflow project
    Init,

    /// Create a new task
    Create {
        /// Task name
        name: String,
        /// Task description (optional)
        description: Option<String>,
        /// Comma-separated list of task dependencies
        #[arg(short, long)]
        depends: Option<String>,
    },

    /// List all tasks
    List,

    /// Start a task
    Start {
        /// Task name
        task: String,
    },

    /// Show task status
    Status {
        /// Task name (optional, shows all if omitted)
        task: Option<String>,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Advance to next step
    Next {
        /// Task name
        task: String,
    },

    /// Retry current step
    Retry {
        /// Task name
        task: String,
    },

    /// Go back to previous step
    Back {
        /// Task name
        task: String,
    },

    /// Skip current step
    Skip {
        /// Task name
        task: String,
    },

    /// Stop a running task
    Stop {
        /// Task name
        task: String,
    },

    /// Reset task to initial state
    Reset {
        /// Task name
        task: String,
    },

    /// Enter task window
    Enter {
        /// Task name
        task: String,
    },

    /// Capture tmux window content
    Capture {
        /// Task name
        task: String,
        /// Number of lines to capture (default: 50)
        #[arg(short, long, default_value = "50")]
        lines: usize,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Wait for task to reach a specific status
    Wait {
        /// Task name
        task: String,
        /// Target status to wait for (pending, running, waiting, completed, failed, stopped)
        #[arg(long)]
        until: String,
        /// Timeout in seconds (default: 300)
        #[arg(short, long, default_value = "300")]
        timeout: u64,
        /// Poll interval in milliseconds (default: 500)
        #[arg(long, default_value = "500")]
        interval: u64,
    },

    /// Show task logs
    Log {
        /// Task name
        task: String,
        /// Show specific step log (1-based index)
        #[arg(short, long)]
        step: Option<usize>,
        /// Show all step logs
        #[arg(short, long)]
        all: bool,
    },

    /// Mark current step as done (for agent use)
    Done {
        /// Task name
        task: String,
        /// Optional message
        #[arg(short, long)]
        message: Option<String>,
    },

    /// Mark current step as failed (for agent use)
    Fail {
        /// Task name
        task: String,
        /// Optional message
        #[arg(short, long)]
        message: Option<String>,
    },

    /// Internal: called on window exit
    #[command(name = "_on-exit", hide = true)]
    OnExit {
        /// Task name
        task: String,
        /// Exit code from the command
        exit_code: i32,
    },

    /// Open interactive TUI
    Tui,
}
