# wf — AI Agent Orchestrator

An orchestrator for AI coding agents. Manages agent lifecycle (setup → develop → verify → merge → cleanup) with git worktree isolation and tmux-based execution.

## Architecture

```
src/
├── main.rs              # Entry point
├── cli.rs               # clap CLI (20 subcommands)
├── model/
│   ├── config.rs        # Config + Step structs, JSONC loader
│   ├── event.rs         # Event enum (12 variants), AgentResult, replay()
│   ├── state.rs         # TaskState, TaskStatus, StepStatus (projection types)
│   └── task.rs          # TaskDefinition + YAML frontmatter parser
├── cmd/
│   ├── mod.rs           # Command dispatch
│   ├── common.rs        # Project context, event append/read/replay helpers
│   ├── init.rs          # wf init (guided TUI)
│   ├── create.rs        # wf create
│   ├── start.rs         # wf start (execution engine core)
│   ├── status.rs        # wf status / wf list
│   ├── control.rs       # wf next/retry/back/skip/stop/reset + _on-exit
│   ├── agent.rs         # wf done/fail/block (with stop_hook validation)
│   ├── capture.rs       # wf capture (tmux content)
│   ├── wait.rs          # wf wait (poll until status)
│   ├── enter.rs         # wf enter (attach to tmux window)
│   └── log.rs           # wf log (--step/--all)
├── tui/
│   ├── app.rs           # Main loop
│   ├── state/           # App state, reducer, per-view state
│   ├── view/            # Layout, style, task_list, task_detail, tmux_pane, popups
│   ├── event/           # Action enum, key input mapping
│   └── data/            # Data provider, live refresh
└── util/
    ├── git.rs           # get_repo_root, validate_branch_name
    ├── shell.rs         # run_command variants, CommandResult
    ├── tmux.rs          # Session/window/pane ops, session_id extraction
    └── variable.rs      # Context struct, expand(), to_env_vars()
```

## Core Concepts

- **Step**: Execution unit — normal command, checkpoint (no `run`), or in_window (`in_window: true`)
- **Checkpoint**: Pauses workflow, waits for `wf next`
- **in_window**: Runs command in tmux window, waits for `wf done/fail/block`
- **Stop Hook**: Validation command on Step; must exit 0 before `wf done` succeeds

## Config (`.wf/config.jsonc`)

```typescript
{
  session?: string,         // tmux session name (default: project dir name)
  multiplexer?: string,     // default: "tmux"
  claude_command?: string,  // default: "claude"
  worktree_dir?: string,    // default: ".wf/worktrees"
  base_branch?: string,     // default: "main"
  workflow: Step[],         // required
  hooks?: Record<string, string>  // event hooks
}

// Step
{
  name: string,             // required
  run?: string,             // shell command (omit for checkpoint)
  in_window?: boolean,      // default: false
  stop_hook?: string        // validation command for wf done
}
```

## Variables

All variables are available as `${var}` in config and as `WF_VAR` env vars in subprocesses.

| Variable | Env Var | Value |
|----------|---------|-------|
| `${task}` | `WF_TASK` | Task name |
| `${branch}` | `WF_BRANCH` | `wf/{task}` |
| `${worktree}` | `WF_WORKTREE` | `{repo_root}/{worktree_dir}/{task}` |
| `${window}` | `WF_WINDOW` | Same as task name |
| `${session}` | `WF_SESSION` | Tmux session name |
| `${repo_root}` | `WF_REPO_ROOT` | Git repository root |
| `${step}` | `WF_STEP` | Current step name |
| `${base_branch}` | `WF_BASE_BRANCH` | Config base_branch value |
| `${log_file}` | `WF_LOG_FILE` | `.wf/logs/{task}.jsonl` |
| `${task_file}` | `WF_TASK_FILE` | `.wf/tasks/{task}.md` |
| `${step_index}` | `WF_STEP_INDEX` | Current step index (0-based) |

## State Machine

**TaskStatus**: `Pending` → `Running` → `Waiting` / `Completed` / `Failed` / `Stopped`

**StepStatus**: `Success` | `Failed` | `Blocked` | `Skipped`

## Event Sourcing

JSONL is the **single source of truth** — no `status.json`. State is reconstructed via `replay()`.

Per-task event log: `.wf/logs/{task}.jsonl`

12 event types:
- `task_started` — initializes Running, step=0
- `command_executed` — exit_code==0 ? Success+advance : Failed
- `checkpoint_reached` — Waiting
- `checkpoint_passed` — advance step
- `window_launched` — Running (in_window step sent to tmux)
- `agent_reported` — Done/Failed/Blocked result from agent
- `on_exit` — tmux exit handler (ignored if AgentReported already handled the step)
- `step_skipped` — Skipped+advance
- `step_retried` — clear step_status, Running
- `step_rolled_back` — current_step=to_step, Waiting
- `task_stopped` — Stopped
- `task_reset` — clears all state (replay restarts)

Auto-completion: when `current_step >= workflow_len`, replay derives `Completed`.

## CLI Commands

| Command | Description |
|---------|-------------|
| `wf init` | Initialize project |
| `wf create <name> [desc] [--depends a,b]` | Create task |
| `wf list` | List all tasks |
| `wf start <task>` | Start task execution |
| `wf status [task] [--json]` | Show status |
| `wf next <task>` | Continue past checkpoint |
| `wf retry <task>` | Retry failed step |
| `wf back <task>` | Go back one step |
| `wf skip <task>` | Skip current step |
| `wf stop <task>` | Stop running task |
| `wf reset <task>` | Reset to initial state |
| `wf enter <task>` | Attach to tmux window |
| `wf capture <task> [-l N] [--json]` | Capture tmux content |
| `wf wait <task> --until <status> [-t sec]` | Wait for status |
| `wf log <task> [--step N] [--all]` | View logs |
| `wf done <task> [-m msg]` | Agent: mark step done |
| `wf fail <task> [-m msg]` | Agent: mark step failed |
| `wf block <task> [-m msg]` | Agent: mark step blocked |
| `wf tui` | Open interactive TUI |

## Execution Flow

```
start(task)
  └─ execute loop:
     ├─ Checkpoint → status=Waiting, return (wait for `wf next`)
     ├─ Normal step → run sync → exit 0? next step : Failed
     └─ in_window step → send to tmux → return (wait for `wf done/fail/block`)

done(task)
  ├─ run stop_hook (if any) → fail? reject
  ├─ append_event(AgentReported { result: Done })
  ├─ continue_execution (run remaining steps)
  └─ cleanup tmux window
```

## File System Layout

```
.wf/
├── config.jsonc          # Workflow configuration
├── tasks/                # Task definitions (markdown + YAML frontmatter)
│   └── {task}.md
├── logs/                 # Event logs (JSONL) — single source of truth
│   └── {task}.jsonl
├── worktrees/            # Git worktrees (one per task)
│   └── {task}/
└── hooks/                # Generated hook files (e.g. settings.json)
```

## Dev Commands

```bash
cargo build               # Build
cargo install --path .     # Install to ~/.cargo/bin
cargo test                 # Run tests
```
