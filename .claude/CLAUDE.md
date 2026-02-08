# pawl — Resumable Step Sequencer

A resumable coroutine: advance along a fixed step sequence, yield when unable to self-decide, rebuild from log after crash. ~4200 lines of Rust powering any multi-step workflow — from AI agent orchestration to deployment pipelines.

## Design Philosophy

### The Single Invariant

```
state = replay(log)
```

Everything else derives from this. Append-only JSONL is the only truth. No caches, no `status.json`, no separate state. Break this and nothing works; hold this and everything follows.

### Three Generative Principles

Pawl's design rules aren't a checklist — they share three generators. Each generator eliminates a class of bugs, not an instance.

**1. Separate what from where** — Observe first, then route. Never mix.

Recording (what happened) and routing (what to do next) are orthogonal. `settle_step()` embodies this: combine → `decide()` → `apply_verdict()`. The `decide()` function is pure — 2 params, 6 rules, zero IO. Recording is unconditional; routing is the only branch.

Corollary: each event type has exactly one emission point. If you find two code paths emitting the same event, one is wrong.

**2. Derive, don't write** — If two artifacts must stay in sync, they're one artifact.

`Display`/`FromStr` replace hand-written format/parse pairs. `Project::context_for()` replaces 4 × 15-line Context constructions. `Project::step_name()` replaces 6+ inline boundary checks. The rule: if you change one and must remember to change another, delete one.

Corollary: the best refactoring deletes code. Every intermediate layer is a decision you're making for the caller — and you make it worse than they would.

**3. Trust the substrate** — Don't build what already works.

The file system is already a database. JSONL is already a state machine. Unix exit codes are already a verdict protocol. `grep` is 50 years old and still sufficient. Build only what the substrate can't do: the coroutine resume semantics, the viewport abstraction, the failure routing algebra. Everything else, delegate.

Corollary: the right abstraction is discovered by deletion, not designed by addition.

### Invariant Rules

From the three generators, five operational rules follow:

1. **Memory = append-only log**. `state = replay(log)`, no separate storage. *(from: trust the substrate)*
2. **Cursor moves forward monotonically**. `current_step` only increases, except on explicit Reset. *(from: separate what from where)*
3. **Verdict before advance**. Cursor advances only after success/failure is determined. *(from: separate what from where)*
4. **Two authorities per decision point**. Machine (exit code) or human (`pawl done`), never both. *(from: derive, don't write — one source of truth per decision)*
5. **Failure is routable**. Failure → retry | yield | terminate. *(from: trust the substrate — exit codes + routing algebra)*

### Coding Conventions

- **Step indexing**: 0-based in all programmatic interfaces. 1-based only in stderr progress.
- **stdout = JSON, stderr = progress**. Write commands output `output_task_state()` JSON. Read commands output JSON/JSONL directly. No `--json`/`--jsonl` flags.
- **Zero warnings**. Dead code is deleted, not suppressed.
- **`cargo install --path .` after build**. PATH uses the installed binary.
- **Tests cover decision logic**: Pure functions (`decide()`) get unit tests. IO is verified via E2E.

## Architecture

```
src/
├── main.rs              # Entry point
├── cli.rs               # clap CLI (14 subcommands)
├── model/
│   ├── config.rs        # Config + Step structs, JSONC loader
│   ├── event.rs         # Event enum (10 variants), replay(), count_auto_retries()
│   ├── state.rs         # TaskState, TaskStatus (Display+FromStr), StepStatus (Display)
│   └── task.rs          # TaskDefinition + YAML frontmatter parser (with skip)
├── cmd/
│   ├── mod.rs           # Command dispatch
│   ├── common.rs        # Project context, event append/read/replay/detect_viewport_loss
│   ├── init.rs          # pawl init (scaffold, uses include_str! for templates)
│   ├── create.rs        # pawl create (improved task template)
│   ├── start.rs         # pawl start (execution engine, settle_step pipeline)
│   ├── status.rs        # pawl status / pawl list
│   ├── control.rs       # pawl stop/reset
│   ├── run.rs           # pawl _run (in_viewport parent process, replaces runner script + trap)
│   ├── done.rs          # pawl done (approve waiting step or complete in_viewport step)
│   ├── capture.rs       # pawl capture (tmux content)
│   ├── wait.rs          # pawl wait (poll via Project API)
│   ├── enter.rs         # pawl enter (attach to viewport)
│   ├── events.rs        # pawl events (unified event stream, --follow)
│   ├── log.rs           # pawl log (--step/--all/--all-runs, JSONL output)
│   └── templates/       # Template files embedded via include_str!
│       ├── config.jsonc           # Default workflow config (self-documented)
│       ├── pawl-skill.md          # SKILL.md: orientation + role routing
│       ├── author.md              # Role: task authoring guide
│       ├── orchestrate.md         # Role: workflow design, recipes, Claude CLI
│       └── supervise.md           # Role: supervisor loop, troubleshooting
├── viewport/
│   ├── mod.rs           # Viewport trait + create_viewport() factory
│   └── tmux.rs          # TmuxViewport implementation
└── util/
    ├── git.rs           # get_repo_root, validate_branch_name, branch_exists
    ├── shell.rs         # run_command variants, CommandResult
    └── variable.rs      # Context (builder pattern), expand(), to_env_vars(), get()
```

## Core Concepts

- **Step**: 4 orthogonal properties: `run`, `verify`, `on_fail`, `in_viewport`
- **Gate step**: No `run` command — waits for `pawl done`
- **in_viewport**: Runs command in viewport, waits for `pawl done`
- **Verify**: `"human"` for manual approval, or a shell command (must exit 0)
- **on_fail**: `"retry"` for auto-retry (up to max_retries), `"human"` to wait for decision
- **skip** (per-task): Task frontmatter `skip: [step_name, ...]` auto-skips listed steps

## Config (`.pawl/config.jsonc`)

```typescript
{
  session?: string,         // tmux session name (default: project dir name)
  viewport?: string,        // default: "tmux"
  worktree_dir?: string,    // default: ".pawl/worktrees"
  base_branch?: string,     // default: "main"
  workflow: Step[],         // required
  on?: Record<string, string>     // event hooks (key = Event serde tag)
}

// Step
{
  name: string,             // required
  run?: string,             // shell command (omit for gate step)
  in_viewport?: boolean,    // default: false
  verify?: string,          // "human" or shell command (must exit 0)
  on_fail?: string,         // "retry" or "human"
  max_retries?: number      // default: 3 (when on_fail="retry")
}
```

## Task Definition (`.pawl/tasks/{task}.md`)

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

All variables are available as `${var}` in config and as `PAWL_VAR` env vars in subprocesses.

| Variable | Env Var | Value |
|----------|---------|-------|
| `${task}` | `PAWL_TASK` | Task name |
| `${branch}` | `PAWL_BRANCH` | `pawl/{task}` |
| `${worktree}` | `PAWL_WORKTREE` | `{repo_root}/{worktree_dir}/{task}` |
| `${session}` | `PAWL_SESSION` | Tmux session name |
| `${repo_root}` | `PAWL_REPO_ROOT` | Git repository root |
| `${step}` | `PAWL_STEP` | Current step name |
| `${base_branch}` | `PAWL_BASE_BRANCH` | Config base_branch value |
| `${log_file}` | `PAWL_LOG_FILE` | `.pawl/logs/{task}.jsonl` |
| `${task_file}` | `PAWL_TASK_FILE` | `.pawl/tasks/{task}.md` |
| `${step_index}` | `PAWL_STEP_INDEX` | Current step index (0-based) |
| `${run_id}` | `PAWL_RUN_ID` | UUID v4 for current run |
| `${retry_count}` | `PAWL_RETRY_COUNT` | Auto-retry count for current step |
| `${last_verify_output}` | `PAWL_LAST_VERIFY_OUTPUT` | Last failure output (verify/stdout/stderr) |

## State Machine

**TaskStatus**: `Pending` → `Running` → `Waiting` / `Completed` / `Failed` / `Stopped`

**StepStatus**: `Success` | `Failed` | `Skipped`

**Step indexing**: All programmatic interfaces use **0-based** step indices (JSONL events, JSON output, `--step` filter, env vars). 1-based only in stderr progress messages (`[1/5] build`).

## Event Sourcing

JSONL is the **single source of truth** — no `status.json`. State is reconstructed via `replay()`.

Per-task event log: `.pawl/logs/{task}.jsonl`

10 event types:
- `task_started` — initializes Running, step=0
- `step_finished` — success=true ? Success+advance : Failed (unified: sync, _run, done, verify failure). Includes `verify_output` for verify failures.
- `step_yielded` — step paused, waiting for approval (reason: "gate"/"verify_human"/"on_fail_human")
- `step_resumed` — approval granted, advance step
- `viewport_launched` — Running (in_viewport step sent to tmux)
- `step_skipped` — Skipped+advance
- `step_reset` — reset step to Running (auto=true for retry, auto=false for manual)
- `task_stopped` — Stopped
- `task_reset` — clears all state (replay restarts)
- `viewport_lost` — viewport disappeared, auto-marked as Failed

Auto-completion: when `current_step >= workflow_len`, replay derives `Completed`.

Event hooks: `config.on` maps event type names to shell commands. Hooks are auto-fired in `append_event()` — no manual trigger needed. Event-specific variables (`${success}`, `${exit_code}`, `${duration}`, `${auto}`, `${reason}`) are injected alongside standard context variables.

## CLI Commands

| Command | Description |
|---------|-------------|
| `pawl init` | Initialize project |
| `pawl create <name> [desc] [--depends a,b]` | Create task |
| `pawl list` | List all tasks |
| `pawl start <task> [--reset]` | Start task execution (--reset auto-resets first) |
| `pawl status [task]` | Show status (JSON to stdout) |
| `pawl stop <task>` | Stop task (Running or Waiting) |
| `pawl reset <task>` | Reset to initial state |
| `pawl reset --step <task>` | Retry current step |
| `pawl enter <task>` | Attach to viewport |
| `pawl capture <task> [-l N]` | Capture tmux content (JSON to stdout) |
| `pawl wait <task> --until <status>[,status2] [-t sec]` | Wait for status (multi-status) |
| `pawl log <task> [--step N] [--all] [--all-runs]` | View logs as JSONL (--all=current run, --all-runs=full history) |
| `pawl events [task] [--follow]` | Unified event stream (--follow for real-time) |
| `pawl done <task> [-m msg]` | Mark step done / approve |

## Execution Flow

```
start(task)
  └─ execute loop:
     ├─ Skip check (task.skip contains step.name) → StepSkipped, continue
     ├─ Gate step (no run) → StepYielded, return (wait for pawl done)
     ├─ Normal step → run sync → settle_step
     └─ in_viewport step → send `pawl _run task step` to tmux → return (wait for pawl done/_run completion)

settle_step(record, step):
  1. combine: (exit_code, verify) → Outcome (Success/HumanNeeded/Failure)
  2. derive_fail_policy: Step config + retry state → FailPolicy
  3. decide(outcome, policy) → Verdict (pure function, 6 rules)
  4. apply_verdict: unconditionally record StepFinished, then route (Advance/Yield/Retry/Fail)

decide(outcome, policy) → Verdict (pure function, 6 rules):
  ├─ Success → Advance
  ├─ HumanNeeded → Yield("verify_human")
  ├─ Failure + Retry(can_retry) → Retry
  ├─ Failure + Retry(!can_retry) → Fail
  ├─ Failure + Human → Yield("on_fail_human")
  └─ Failure + Terminal → Fail

done(task)
  ├─ Running: settle_step (emits StepFinished inside)
  │   └─ retry? keep viewport : kill viewport
  └─ Waiting: emit StepResumed → resume_workflow

_run(task, step_idx)  [runs inside viewport as parent process]
  ├─ ignore SIGHUP, fork child (bash -c command), waitpid
  ├─ redirect stdout/stderr → /dev/null (pty may be gone)
  ├─ re-check state (pawl done may have already handled)
  └─ if still Running at step_idx → settle_step

detect_viewport_loss(task_name) → bool:
  └─ Running + in_viewport + viewport gone → emit ViewportLost, return false
```

## File System Layout

```
.pawl/
├── config.jsonc              # Workflow configuration (self-documented)
├── tasks/                    # Task definitions (markdown + YAML frontmatter)
│   └── {task}.md
├── logs/                     # Event logs (JSONL) — single source of truth
│   └── {task}.jsonl
├── skills/pawl/              # Skill reference (pawl init generates)
│   ├── SKILL.md              # Orientation + role routing
│   └── references/           # Role-specific guides (progressive disclosure)
│       ├── author.md         # Writing effective tasks
│       ├── orchestrate.md    # Designing workflows + Claude Code CLI
│       └── supervise.md      # Monitoring, status decisions, troubleshooting
└── worktrees/                # Git worktrees (one per task)
    └── {task}/
```

## Dev Commands

```bash
cargo build               # Build
cargo install --path .     # Install to ~/.cargo/bin
cargo test                 # Run tests
```
