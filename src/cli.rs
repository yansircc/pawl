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
        /// Reset task before starting (auto reset+start in one step)
        #[arg(long)]
        reset: bool,
    },

    /// Show task status
    Status {
        /// Task name (optional, shows all if omitted)
        task: Option<String>,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Stop a running task
    Stop {
        /// Task name
        task: String,
    },

    /// Reset task (full reset, or --step to retry current step)
    Reset {
        /// Task name
        task: String,
        /// Only reset current step (retry) instead of full task reset
        #[arg(long)]
        step: bool,
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
        /// Show specific step log (0-based index)
        #[arg(short, long)]
        step: Option<usize>,
        /// Show all events in the current run
        #[arg(short, long)]
        all: bool,
        /// Show all events across all runs (including before resets)
        #[arg(long)]
        all_runs: bool,
        /// Output raw JSONL (pipe to jq for queries)
        #[arg(long)]
        jsonl: bool,
    },

    /// Stream events from all (or specified) tasks in real-time
    Events {
        /// Only stream events for this task (optional, streams all if omitted)
        task: Option<String>,
        /// Keep streaming (tail -f mode). Without this, prints existing events and exits.
        #[arg(short, long)]
        follow: bool,
    },

    /// Mark current step as done / approve waiting step
    Done {
        /// Task name
        task: String,
        /// Optional message
        #[arg(short, long)]
        message: Option<String>,
    },

    /// Internal: run command in tmux window as parent process
    #[command(name = "_run", hide = true)]
    Run {
        /// Task name
        task: String,
        /// Step index (0-based)
        step: usize,
    },
}
