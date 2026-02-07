# wf — AI Agent Orchestrator

An orchestrator for AI coding agents. Manages agent lifecycle (setup → develop → verify → merge → cleanup) with git worktree isolation and tmux-based execution.

## Design Philosophy

### System Essence

wf is a **resumable coroutine**: advance along a fixed sequence, yield control when unable to self-decide, rebuild from log after crash.

### 5 Invariant Rules

1. **Memory = append-only log**. `state = replay(log)`, no separate state storage.
2. **Cursor moves forward monotonically**. `current_step` only increases, except on explicit Reset.
3. **Verdict before advance**. Cursor advances only after success/failure of current position is determined.
4. **Two authorities per decision point**. Each step's outcome is decided by either machine (exit code) or human (`wf done`), never both. Environment (WindowLost) is anomaly detection, not a verdict.
5. **Failure is routable**. Failure → retry (Reset) | yield to human (Yield) | terminate.

**The single invariant**: `state = replay(log)`

### Architecture Principles

- **Decision/IO separation**: `resolve()` is a pure function (no side effects), `dispatch()` handles all IO. Never mix decision logic with event emission.
- **Single emission point**: Each event type should have exactly one code path that emits it. WindowLost → `check_window_health()`. StepCompleted → `dispatch()`.
- **All state from replay**: Never cache or store state separately. Always call `replay_task()` to get current state.
- **Immutable Project**: All cmd functions take `&Project` (not `&mut`). State changes happen through `append_event()` → re-`replay()`.
- **Event minimalism**: 10 events = 4 primitives (Advance/Yield/Fail/Reset) + 2 lifecycle + 1 observation. Do not add events without proving the existing set cannot represent the semantics.

### Coding Conventions

- **Step indexing**: 0-based in all programmatic interfaces (JSONL, `--json`, env vars). 1-based only in human-facing CLI output.
- **`cargo build` must produce zero warnings**. Dead code should be deleted, not suppressed.
- **`cargo install --path .` after build**. PATH uses the installed binary, not the build artifact.
- **Tests cover decision logic**: Pure functions (like `resolve()`) get unit tests. IO functions are verified via E2E.

## Architecture

```
src/
├── main.rs              # Entry point
├── cli.rs               # clap CLI (14 subcommands)
├── model/
│   ├── config.rs        # Config + Step structs, JSONC loader
│   ├── event.rs         # Event enum (10 variants), replay(), count_auto_retries()
│   ├── state.rs         # TaskState, TaskStatus, StepStatus (projection types)
│   └── task.rs          # TaskDefinition + YAML frontmatter parser (with skip)
├── cmd/
│   ├── mod.rs           # Command dispatch
│   ├── common.rs        # Project context, event append/read/replay/check_window_health
│   ├── init.rs          # wf init (scaffold, uses include_str! for templates)
│   ├── create.rs        # wf create (improved task template)
│   ├── start.rs         # wf start (execution engine, resolve/dispatch pipeline)
│   ├── status.rs        # wf status / wf list
│   ├── control.rs       # wf stop/reset + _on-exit
│   ├── approve.rs       # wf done (approve waiting step or complete in_window step)
│   ├── capture.rs       # wf capture (tmux content)
│   ├── wait.rs          # wf wait (poll via Project API)
│   ├── enter.rs         # wf enter (attach to tmux window)
│   ├── events.rs        # wf events (unified event stream, --follow)
│   ├── log.rs           # wf log (--step/--all/--all-runs/--jsonl)
│   └── templates/       # Template files embedded via include_str!
│       ├── config.jsonc           # Default workflow config
│       ├── ai-helpers.sh          # AI worker helper functions
│       ├── foreman-guide.md       # Foreman operation manual
│       ├── task-authoring-guide.md # Task.md writing guide
│       └── ai-worker-guide.md     # AI worker integration guide
└── util/
    ├── git.rs           # get_repo_root, validate_branch_name, branch_exists
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
| `wf stop <task>` | Stop task (Running or Waiting) |
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
     ├─ Normal step → run sync → handle_step_completion
     └─ in_window step → send to tmux → return (wait for wf done)

handle_step_completion(exit_code, step, run_output):
  1. run_verify (if exit_code == 0) → VerifyOutcome
  2. count_auto_retries → retry_count
  3. resolve(exit_code, verify_outcome, on_fail, retry_count, max_retries) → Action
  4. dispatch(action) → emit events + IO

resolve() → Action (pure function, 7 paths):
  ├─ exit_code=0, verify Passed → Advance
  ├─ exit_code=0, verify HumanRequired → YieldVerifyHuman
  ├─ exit_code=0, verify Failed, no on_fail → Fail
  ├─ exit_code=0, verify Failed, retry under limit → Retry
  ├─ exit_code=0, verify Failed, retry at limit → Fail
  ├─ exit_code=0, verify Failed, human → YieldOnFailHuman
  └─ exit_code!=0, no on_fail → Fail (+ retry/human variants)

done(task)
  ├─ Running: handle_step_completion (emits StepCompleted inside)
  │   └─ retry? keep tmux window : kill tmux window
  └─ Waiting: emit StepApproved → continue

on_exit(task, exit_code)
  ├─ if exit_code==0 && in_window → check_window_health (may emit WindowLost)
  └─ else → handle_step_completion (emits StepCompleted inside)

check_window_health(task_name) → bool:
  └─ Running + in_window + window gone → emit WindowLost, return false
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
└── lib/                  # Helper library
    ├── ai-helpers.sh     # AI worker functions (extract_session_id, run_ai_worker)
    ├── foreman-guide.md  # Foreman agent operation manual (with JSON schema, decision table)
    ├── task-authoring-guide.md  # Task.md writing guide (dual purpose, feedback iteration)
    └── ai-worker-guide.md      # AI worker integration guide (wrapper.sh, session resumption)
```

## Dev Commands

```bash
cargo build               # Build
cargo install --path .     # Install to ~/.cargo/bin
cargo test                 # Run tests
```
