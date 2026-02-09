# pawl — Durable Execution Primitive for Shell

Shell's missing `yield`. A single binary that turns any shell pipeline into a resumable coroutine with failure routing. Define a step sequence, run it for any task — pawl yields when it needs judgment, retries on failure, and rebuilds state from log after crash. The consumer is agents (AI or script), not humans. CLI output is JSON stdout + plain text stderr. `pawl status` provides self-routing hints (suggest/prompt) so agents know what to do next without understanding pawl's internals.

## Design Philosophy

### The Single Invariant

```
state = replay(log)
```

Append-only JSONL is the only truth. No caches, no `status.json`, no separate state. Break this and nothing works; hold this and everything follows.

### Three Generative Principles

1. **Separate what from where** — Recording (what happened) and routing (what to do next) are orthogonal. Each event type has exactly one emission point — two code paths emitting the same event means one is wrong.
2. **Derive, don't write** — If two artifacts must stay in sync, they're one artifact. If you change one and must remember to change another, delete one. The best refactoring deletes code.
3. **Trust the substrate** — File system, JSONL, Unix exit codes, tmux — don't rebuild what already works. Build only coroutine semantics, viewport abstraction, failure routing algebra.

**Stop condition: less-is-more** — If no code path consumes a structure, delete it. Each mechanism exists only in its responsibility scope: errors report, status routes, write commands confirm. Crossing scope boundaries creates redundant emission points.

### Invariant Rules

1. **Memory = append-only log**. No separate storage.
2. **Cursor moves forward monotonically**. Except on explicit Reset.
3. **Verdict before advance**. Cursor advances only after success/failure is determined.
4. **Two authorities per decision point**. Machine (exit code) or manual (`pawl done`), never both.
5. **Failure is routable**. Failure → retry | yield | terminate.

### Agent-First Interface

1. **Structured output**: stdout = JSON/JSONL, stderr = progress. JSON is the only format, not an option.
2. **Typed errors**: `PawlError` → plain text stderr + exit codes 2-7 (StateConflict=2, Precondition=3, NotFound=4, AlreadyExists=5, Validation=6, Timeout=7). Internal errors remain anyhow (exit 1).
3. **Status-driven routing**: `pawl status` includes `suggest` (mechanical) and `prompt` (requires judgment). Routing lives only in status — errors report, they don't route.
4. **Agent UX ≠ Human UX**: Agents consume SKILL.md, not `-h`.

### Coding Conventions

- **Step indexing**: 0-based in all programmatic interfaces. 1-based only in stderr progress.
- **stdout = JSON, stderr = progress**. Task identifier field is `"name"` (not `"task"`).
- **Zero warnings**. Dead code is deleted, not suppressed.
- **`cargo install --path .` after build**. PATH uses the installed binary.
- **Tests cover decision logic**: Pure functions (`decide()`) get unit tests. IO is verified via E2E.

## Architecture

```
src/
├── main.rs              # Entry point, PawlError → text stderr + exit code
├── cli.rs               # clap CLI (11 subcommands)
├── error.rs             # PawlError enum (6 variants, exit codes 2-7)
├── model/
│   ├── config.rs        # Config + TaskConfig + Step structs, JSON loader, vars (IndexMap)
│   ├── event.rs         # Event enum (10 variants), replay(), count_auto_retries()
│   └── state.rs         # TaskState, TaskStatus (Display+FromStr), StepStatus (Display)
├── cmd/
│   ├── mod.rs           # Command dispatch
│   ├── common.rs        # Project context, event IO, output_task_state
│   ├── init.rs          # pawl init (config.json + README.md scaffold)
│   ├── start.rs         # pawl start (execution engine, settle_step pipeline)
│   ├── status.rs        # pawl status / pawl list (+ derive_routing for suggest/prompt)
│   ├── control.rs       # pawl stop/reset
│   ├── run.rs           # pawl _run (in_viewport parent process)
│   ├── done.rs          # pawl done (approve waiting step or complete in_viewport step)
│   ├── wait.rs          # pawl wait (poll via Project API)
│   ├── events.rs        # pawl events (unified event stream, --follow, --type filter)
│   ├── log.rs           # pawl log (--step/--all, JSONL output)
│   └── templates/       # Template files embedded via include_str!
│       ├── config.json            # Empty scaffold
│       └── readme.md              # README.md: pawl reference
├── viewport/
│   ├── mod.rs           # Viewport trait (open/execute/exists/close)
│   └── tmux.rs          # TmuxViewport implementation
└── util/
    ├── project.rs       # get_project_root (.pawl/ walk-up)
    ├── shell.rs         # run_command variants, CommandResult
    └── variable.rs      # Context (builder pattern), expand(), to_env_vars()
```

## Core Concepts

- **Step**: 4 orthogonal properties: `run`, `verify`, `on_fail`, `in_viewport`
- **Gate step**: No `run` command — waits for `pawl done`
- **in_viewport**: Runs command in viewport, waits for `pawl done`
- **Verify**: `"manual"` for manual approval, or a shell command (must exit 0)
- **on_fail**: `"retry"` for auto-retry (up to max_retries), `"manual"` to wait for decision
- **skip** (per-task): `config.json` tasks section `"skip": ["step_name"]` auto-skips listed steps

## State Machine

**TaskStatus**: `Pending` → `Running` → `Waiting` / `Completed` / `Failed` / `Stopped`

**StepStatus**: `Success` | `Failed` | `Skipped`

## Event Sourcing

JSONL per-task log (`.pawl/logs/{task}.jsonl`) is the **single source of truth**. State is reconstructed via `replay()`.

10 event types: `task_started`, `step_finished`, `step_yielded`, `step_resumed`, `viewport_launched`, `step_skipped`, `step_reset`, `task_stopped`, `task_reset`, `viewport_lost`.

Auto-completion: `current_step >= workflow_len` → replay derives `Completed`. Event hooks: `config.on` maps event type names to shell commands, auto-fired in `append_event()`.

## Dev & Test

```bash
cargo build && cargo install --path .    # Build + install
cargo test                               # Unit tests (decide() logic)
```

E2E tests (require tmux):

| Script | Tests | What | Cost |
|--------|-------|------|------|
| `tests/e2e.sh` | 68 | Sync workflows | Free |
| `tests/e2e-viewport.sh` | 27 | Viewport lifecycle | Free |
| `tests/e2e-agent.sh` | 9 | Real haiku agents | ~$0.05 |

## Session Start

1. Read `.claude/HANDOFF.md` — 上次 session 做了什么、pending 问题、key file index
2. Read `MEMORY.md` — 实战教训和陷阱（viewport 时序、pipe 退出码、测试踩坑等）
3. `git status` + `git log --oneline -5` — 当前状态

## Common Traps

- **类比推理**：不要因为 A 做了 X 就假设 B 也该做 X。先问 A **为什么**做 X，再看 B 是否有相同的 why。
- **职责越界**：每个机制只在自己的 scope 内操作。当你想在 A 里加 B 的逻辑时，问"这是 A 的职责吗？"
