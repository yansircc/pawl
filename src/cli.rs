use clap::{Parser, Subcommand};

/// Resumable step sequencer
#[derive(Parser)]
#[command(name = "pawl", version, after_help = r#"STATES: Pending → Running → Waiting / Completed / Failed / Stopped

OUTPUT: stdout = JSON (write cmds) or JSONL (log/events). stderr = plain text.
INDEXING: 0-based in all programmatic output. 1-based only in stderr progress.

FILES:
  .pawl/config.jsonc      Workflow config — step properties, variables, event hooks
  .pawl/tasks/{task}.md   Task definition (pawl create)
  .pawl/logs/{task}.jsonl  Event log — single source of truth"#)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new workflow project
    Init,

    /// Create a new task
    #[command(after_help = r#"FRONTMATTER: name, depends (list), skip (list of step names to auto-skip).

DUAL PURPOSE: task definition for pawl + AI worker prompt (cat ${task_file} | agent -p).

ON RETRY: append fix guidance to end of task file (don't overwrite — preserves history)."#)]
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
    #[command(after_help = r#"Fields: name, status, current_step (0-based), total_steps,
step_name, message, blocked_by, retry_count, last_feedback, suggest, prompt.
With task arg: adds description, depends, workflow[{index, name, status, step_type}].
suggest = mechanical recovery commands. prompt = requires judgment.
Optional fields omitted when empty/null."#)]
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
    #[command(after_help = "Full reset: clears all state → Pending.\n--step: retries current step (keeps history, useful after failure).")]
    Reset {
        /// Task name
        task: String,
        /// Only reset current step (retry) instead of full task reset
        #[arg(long)]
        step: bool,
    },

    /// Enter task viewport
    Enter {
        /// Task name
        task: String,
    },

    /// Capture viewport content
    Capture {
        /// Task name
        task: String,
        /// Number of lines to capture (default: 50)
        #[arg(short, long, default_value = "50")]
        lines: usize,
    },

    /// Wait for task to reach a specific status
    #[command(after_help = "Comma-separated: --until waiting,failed. Exits 0 on match, 1 on timeout.")]
    Wait {
        /// Task name
        task: String,
        /// Target status (pending, running, waiting, completed, failed, stopped). Comma-separated for multiple.
        #[arg(long)]
        until: String,
        /// Timeout in seconds (default: 300)
        #[arg(short, long, default_value = "300")]
        timeout: u64,
        /// Poll interval in milliseconds (default: 500)
        #[arg(long, default_value = "500")]
        interval: u64,
    },

    /// Show task logs (JSONL output)
    #[command(after_help = "--all: current run events. --all-runs: full history.\n--step N: specific step (0-based). Output is JSONL (pipe to jq).")]
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
    },

    /// Stream events from all (or specified) tasks in real-time
    #[command(after_help = "Without --follow: prints existing and exits. With --follow: tails continuously.\n--type: comma-separated event types (e.g. step_finished,step_yielded).")]
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
    #[command(after_help = "Waiting → approve (step advances).\nRunning + in_viewport → mark done (triggers verify/on_fail flow).")]
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
