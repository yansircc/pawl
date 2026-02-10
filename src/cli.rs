use clap::{Parser, Subcommand};

/// Resumable step sequencer
#[derive(Parser)]
#[command(name = "pawl", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new workflow project
    Init,

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

    /// Wait for task(s) to reach a specific status
    Wait {
        /// Task name(s)
        tasks: Vec<String>,
        /// Target status (pending, running, waiting, completed, failed, stopped). Comma-separated for multiple.
        #[arg(long)]
        until: String,
        /// Timeout in seconds (default: 300)
        #[arg(short, long, default_value = "300")]
        timeout: u64,
        /// Poll interval in milliseconds (default: 500)
        #[arg(long, default_value = "500")]
        interval: u64,
        /// Return when ANY task reaches target (default: wait for ALL)
        #[arg(long)]
        any: bool,
    },

    /// Show task logs (JSONL output)
    Log {
        /// Task name
        task: String,
        /// Show specific step log (0-based index)
        #[arg(short, long)]
        step: Option<usize>,
        /// Show all events in the current run
        #[arg(short, long)]
        all: bool,
    },

    /// Stream events from all (or specified) tasks in real-time
    Events {
        /// Only stream events for this task (optional, streams all if omitted)
        task: Option<String>,
        /// Keep streaming (tail -f mode)
        #[arg(short, long)]
        follow: bool,
        /// Filter by event type (comma-separated, e.g. step_finished,step_yielded)
        #[arg(long = "type")]
        event_type: Option<String>,
    },

    /// Mark current step as done / approve waiting step
    Done {
        /// Task name
        task: String,
        /// Optional message
        #[arg(short, long)]
        message: Option<String>,
    },

    /// Internal: run command in viewport as parent process
    #[command(name = "_run", hide = true)]
    Run {
        /// Task name
        task: String,
        /// Step index (0-based)
        step: usize,
    },
}
