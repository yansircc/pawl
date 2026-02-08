use clap::{Parser, Subcommand};

/// Resumable step sequencer
#[derive(Parser)]
#[command(name = "pawl", version, after_help = r#"STEP PROPERTIES (4 orthogonal):
  run          Shell command (omit for gate — waits for `pawl done`)
  verify       "human" (manual approval) or shell command (exit 0 = pass)
  on_fail      "retry" (auto, up to max_retries) or "human" (yield for decision)
  in_viewport  Run in tmux window, complete via `pawl done` or exit code

STATES: Pending → Running → Waiting / Completed / Failed / Stopped

VARIABLES (${var} in config, PAWL_VAR in subprocesses):
  task, branch (pawl/{task}), worktree, session, repo_root,
  step, step_index (0-based), base_branch, log_file, task_file

INDEXING: 0-based in programmatic interfaces. 1-based in human-readable output.

FILES:
  .pawl/config.jsonc      Workflow config (pawl init)
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
    #[command(after_help = "Steps: skip → gate → sync run → viewport.\nsettle_step: combine(exit_code, verify) → decide(outcome, policy) → apply_verdict.")]
    Start {
        /// Task name
        task: String,
        /// Reset task before starting (auto reset+start in one step)
        #[arg(long)]
        reset: bool,
    },

    /// Show task status
    #[command(after_help = r#"--json fields: name, status, current_step (0-based), total_steps,
step_name, message, blocked_by, retry_count, last_feedback.
With task arg: adds description, depends, workflow[{index, name, status, step_type}].
retry_count = auto retries only. last_feedback stops at task_reset.
step_type: "gate" / "in_viewport" / omitted. Optional fields omitted when null."#)]
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
        /// Output in JSON format
        #[arg(long)]
        json: bool,
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

    /// Show task logs
    #[command(after_help = "--all: current run events. --all-runs: full history.\n--step N: specific step (0-based). --jsonl: raw JSONL (pipe to jq).")]
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
    #[command(after_help = "Without --follow: prints existing and exits. With --follow: tails continuously.")]
    Events {
        /// Only stream events for this task (optional, streams all if omitted)
        task: Option<String>,
        /// Keep streaming (tail -f mode)
        #[arg(short, long)]
        follow: bool,
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
