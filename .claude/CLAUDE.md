# wf — AI Agent Orchestrator

An orchestrator for AI coding agents. Manages agent lifecycle (setup → develop → verify → merge → cleanup) with git worktree isolation and tmux-based execution.

## Architecture

```
src/
├── main.rs              # Entry point
├── cli.rs               # clap CLI (14 subcommands)
├── model/
│   ├── config.rs        # Config + Step structs, JSONC loader
│   ├── event.rs         # Event enum (11 variants), replay()
│   ├── state.rs         # TaskState, TaskStatus, StepStatus (projection types)
│   └── task.rs          # TaskDefinition + YAML frontmatter parser (with skip)
├── cmd/
│   ├── mod.rs           # Command dispatch
│   ├── common.rs        # Project context, event append/read/replay helpers
│   ├── init.rs          # wf init (scaffold + lib template)
│   ├── create.rs        # wf create
│   ├── start.rs         # wf start (execution engine, unified pipeline, verify helpers)
│   ├── status.rs        # wf status / wf list
│   ├── control.rs       # wf stop/reset + _on-exit
│   ├── approve.rs       # wf done (approve waiting step or complete in_window step)
│   ├── capture.rs       # wf capture (tmux content)
│   ├── wait.rs          # wf wait (poll until status)
│   ├── enter.rs         # wf enter (attach to tmux window)
│   ├── events.rs        # wf events (unified event stream, --follow)
│   └── log.rs           # wf log (--step/--all/--all-runs/--jsonl)
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

**Step indexing**: All programmatic interfaces use **0-based** step indices (JSONL events, `--json` output, `--step` filter, env vars). CLI human-readable output uses 1-based (`[1/5] build`).

## Event Sourcing

JSONL is the **single source of truth** — no `status.json`. State is reconstructed via `replay()`.

Per-task event log: `.wf/logs/{task}.jsonl`

10 event types:
- `task_started` — initializes Running, step=0
- `step_completed` — exit_code==0 ? Success+advance : Failed (unified: sync, on_exit, done, verify failure)
- `step_waiting` — step paused, waiting for approval (reason: "gate"/"verify_human"/"on_fail_human")
- `step_approved` — approval granted, advance step
- `window_launched` — Running (in_window step sent to tmux)
- `step_skipped` — Skipped+advance
- `step_reset` — reset step to Running (auto=true for retry, auto=false for manual)
- `task_stopped` — Stopped
- `task_reset` — clears all state (replay restarts)
- `window_lost` — tmux window disappeared, auto-marked as Failed

Auto-completion: when `current_step >= workflow_len`, replay derives `Completed`.

Event hooks: `config.on` maps event type names to shell commands. Hooks are auto-fired in `append_event()` — no manual trigger needed. Event-specific variables (`${exit_code}`, `${duration}`, `${auto}`, `${reason}`) are injected alongside standard context variables.

## CLI Commands

| Command | Description |
|---------|-------------|
| `wf init` | Initialize project |
| `wf create <name> [desc] [--depends a,b]` | Create task |
| `wf list` | List all tasks |
| `wf start <task> [--reset]` | Start task execution (--reset auto-resets first) |
| `wf status [task] [--json]` | Show status (--json uses 0-based step index) |
| `wf stop <task>` | Stop running task |
| `wf reset <task>` | Reset to initial state |
| `wf reset --step <task>` | Retry current step |
| `wf enter <task>` | Attach to tmux window |
| `wf capture <task> [-l N] [--json]` | Capture tmux content |
| `wf wait <task> --until <status>[,status2] [-t sec]` | Wait for status (multi-status) |
| `wf log <task> [--step N] [--all] [--all-runs] [--jsonl]` | View logs (--all=current run, --all-runs=full history) |
| `wf events [task] [--follow]` | Unified event stream (--follow for real-time) |
| `wf done <task> [-m msg]` | Mark step done / approve |

## Execution Flow

```
start(task)
  └─ execute loop:
     ├─ Skip check (task.skip contains step.name) → StepSkipped, continue
     ├─ Gate step (no run) → StepWaiting, return (wait for wf done)
     ├─ Normal step → run sync → handle_step_completion:
     │   ├─ exit_code != 0 → StepCompleted(exit) → apply_on_fail
     │   ├─ no verify → StepCompleted(0) → advance
     │   ├─ verify: "human" → StepCompleted(0) + StepWaiting(verify_human)
     │   └─ verify: command → run it
     │       ├─ pass → StepCompleted(0) → advance
     │       └─ fail → StepCompleted(1, stderr=feedback) → apply_on_fail
     └─ in_window step → send to tmux → return (wait for wf done)

done(task)
  ├─ Running: handle_step_completion (emits StepCompleted inside)
  │   └─ retry? keep tmux window : kill tmux window
  └─ Waiting: emit StepApproved → continue

on_exit(task, exit_code)
  ├─ if exit_code==0 && in_window && window gone → WindowLost (P12 fix)
  └─ else → handle_step_completion (emits StepCompleted inside)

apply_on_fail(strategy):
  ├─ on_fail="retry" → StepReset{auto:true} → continue (up to max_retries)
  ├─ on_fail="human" → StepWaiting (wait for wf done)
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
├── hooks/                # Generated hook files (e.g. settings.json)
└── lib/                  # Shell helper library (ai-helpers.sh)
```

## Dev Commands

```bash
cargo build               # Build
cargo install --path .     # Install to ~/.cargo/bin
cargo test                 # Run tests
```
