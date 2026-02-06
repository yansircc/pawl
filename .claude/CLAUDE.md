# wf — AI Agent Orchestrator

An orchestrator for AI coding agents. Manages agent lifecycle (setup → develop → verify → merge → cleanup) with git worktree isolation and tmux-based execution.

## Architecture

```
src/
├── main.rs              # Entry point
├── cli.rs               # clap CLI (13 subcommands)
├── model/
│   ├── config.rs        # Config + Step structs, JSONC loader
│   ├── event.rs         # Event enum (11 variants), replay()
│   ├── state.rs         # TaskState, TaskStatus, StepStatus (projection types)
│   └── task.rs          # TaskDefinition + YAML frontmatter parser (with skip)
├── cmd/
│   ├── mod.rs           # Command dispatch
│   ├── common.rs        # Project context, event append/read/replay helpers
│   ├── init.rs          # wf init (guided TUI)
│   ├── create.rs        # wf create
│   ├── start.rs         # wf start (execution engine, unified pipeline, verify helpers)
│   ├── status.rs        # wf status / wf list
│   ├── control.rs       # wf stop/reset + _on-exit
│   ├── approve.rs       # wf done (approve waiting step or complete in_window step)
│   ├── capture.rs       # wf capture (tmux content)
│   ├── wait.rs          # wf wait (poll until status)
│   ├── enter.rs         # wf enter (attach to tmux window)
│   └── log.rs           # wf log (--step/--all)
└── util/
    ├── git.rs           # get_repo_root, validate_branch_name
    ├── shell.rs         # run_command variants, CommandResult
    ├── tmux.rs          # Session/window/pane ops, session_id extraction
    └── variable.rs      # Context struct, expand(), to_env_vars()
```

## Core Concepts

- **Step**: 4 orthogonal properties: `run`, `verify`, `on_fail`, `in_window`
- **Gate step**: No `run` command — waits for `wf done`
- **in_window**: Runs command in tmux window, waits for `wf done`
- **Verify**: `"human"` for manual approval, or a shell command (must exit 0)
- **on_fail**: `"retry"` for auto-retry (up to max_retries), `"human"` to wait for decision
- **skip** (per-task): Task frontmatter `skip: [step_name, ...]` auto-skips listed steps

## Config (`.wf/config.jsonc`)

```typescript
{
  session?: string,         // tmux session name (default: project dir name)
  multiplexer?: string,     // default: "tmux"
  claude_command?: string,  // default: "claude"
  worktree_dir?: string,    // default: ".wf/worktrees"
  base_branch?: string,     // default: "main"
  workflow: Step[],         // required
  on?: Record<string, string>     // event hooks (key = Event serde tag)
}

// Step
{
  name: string,             // required
  run?: string,             // shell command (omit for gate step)
  in_window?: boolean,      // default: false
  verify?: string,          // "human" or shell command (must exit 0)
  on_fail?: string,         // "retry" or "human"
  max_retries?: number      // default: 3 (when on_fail="retry")
}
```

## Task Definition (`.wf/tasks/{task}.md`)

```yaml
---
name: my-task
depends:
  - other-task
skip:
  - cleanup        # skip this workflow step for this task
---

Task description in markdown.
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

**StepStatus**: `Success` | `Failed` | `Skipped`

## Event Sourcing

JSONL is the **single source of truth** — no `status.json`. State is reconstructed via `replay()`.

Per-task event log: `.wf/logs/{task}.jsonl`

11 event types:
- `task_started` — initializes Running, step=0
- `step_completed` — exit_code==0 ? Success+advance : Failed (unified: sync, on_exit, done)
- `step_waiting` — step paused, waiting for approval
- `step_approved` — approval granted, advance step
- `window_launched` — Running (in_window step sent to tmux)
- `step_skipped` — Skipped+advance
- `step_reset` — reset step to Running (auto=true for retry, auto=false for manual)
- `task_stopped` — Stopped
- `task_reset` — clears all state (replay restarts)
- `verify_failed` — verify command failed (feedback stored)
- `window_lost` — tmux window disappeared, auto-marked as Failed

Auto-completion: when `current_step >= workflow_len`, replay derives `Completed`.

Event hooks: `config.on` maps event type names to shell commands. Hooks are auto-fired in `append_event()` — no manual trigger needed. Event-specific variables (`${exit_code}`, `${duration}`, `${auto}`, `${feedback}`) are injected alongside standard context variables.

## CLI Commands

| Command | Description |
|---------|-------------|
| `wf init` | Initialize project |
| `wf create <name> [desc] [--depends a,b]` | Create task |
| `wf list` | List all tasks |
| `wf start <task>` | Start task execution |
| `wf status [task] [--json]` | Show status |
| `wf stop <task>` | Stop running task |
| `wf reset <task>` | Reset to initial state |
| `wf reset --step <task>` | Retry current step |
| `wf enter <task>` | Attach to tmux window |
| `wf capture <task> [-l N] [--json]` | Capture tmux content |
| `wf wait <task> --until <status> [-t sec]` | Wait for status |
| `wf log <task> [--step N] [--all]` | View logs |
| `wf done <task> [-m msg]` | Mark step done / approve |

## Execution Flow

```
start(task)
  └─ execute loop:
     ├─ Skip check (task.skip contains step.name) → StepSkipped, continue
     ├─ Gate step (no run) → StepWaiting, return (wait for wf done)
     ├─ Normal step → run sync → StepCompleted → handle_step_completion:
     │   ├─ exit_code != 0 → apply_on_fail(on_fail strategy)
     │   ├─ no verify → advance
     │   ├─ verify: "human" → StepWaiting (wait for wf done)
     │   └─ verify: command → run it
     │       ├─ pass → advance
     │       └─ fail → apply_on_fail(on_fail strategy)
     └─ in_window step → send to tmux → return (wait for wf done)

done(task)
  ├─ Running: run verify → pass? emit StepCompleted → continue
  └─ Waiting: emit StepApproved → continue

on_exit(task, exit_code)
  └─ emit StepCompleted → handle_step_completion

apply_on_fail(strategy):
  ├─ on_fail="retry" → StepReset{auto:true} → continue (up to max_retries)
  ├─ on_fail="human" → StepWaiting (wait for wf done)
  ├─ verify="human" → StepWaiting (wait for wf done)
  └─ default → stay Failed
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
