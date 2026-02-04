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

    /// Show task logs
    Log {
        /// Task name
        task: String,
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

    /// Mark current step as blocked (for agent use)
    Block {
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
    },
}
