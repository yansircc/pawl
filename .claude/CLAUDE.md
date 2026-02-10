# pawl

Shell's missing `yield`. Resumable coroutines with failure routing. Consumers are programs, not humans.

## Design Constraints

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

### Machine-First Interface

1. **Structured output**: stdout = JSON/JSONL, stderr = progress. JSON is the only format, not an option.
2. **Typed errors**: `PawlError` → plain text stderr + exit codes 2-7 (StateConflict=2, Precondition=3, NotFound=4, AlreadyExists=5, Validation=6, Timeout=7). Internal errors remain anyhow (exit 1).
3. **Status-driven routing**: `pawl status` includes `suggest` (mechanical) and `prompt` (requires judgment). Routing lives only in status — errors report, they don't route.
4. **Machine UX ≠ Human UX**: Consumers parse JSON and exit codes, not `-h` text.

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
├── cli.rs               # clap CLI (12 subcommands)
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
│   ├── serve.rs         # pawl serve (tiny_http JSON API + optional --ui)
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

## Model Constraints

- **Step** has exactly 4 orthogonal properties: `run`, `verify`, `on_fail`, `in_viewport`. Don't add a 5th — compose from these.
- **Gate step** (no `run`): no implicit behavior. `verify`/`on_fail` are ignored on gates.
- **in_viewport**: viewport lifecycle only. pawl doesn't know what runs inside.
- **on_fail**: only `"retry"` or `"manual"`. No other failure policies.
- **TaskStatus**: `Pending` → `Running` → `Waiting` / `Completed` / `Failed` / `Stopped`. No other states.
- **StepStatus**: `Success` | `Failed` | `Skipped`. No other outcomes.
- **Events**: 10 types, each with exactly one emission point. Adding an event type is a design decision, not a code decision.
- **Replay**: `.pawl/logs/{task}.jsonl` → `replay()` → `TaskState`. Auto-completion: `current_step >= workflow_len` → `Completed`.

## Execution Engine

### settle_step pipeline: combine → decide → split

1. **Combine**: exit_code + verify → `Outcome` (Success | Failure{feedback} | ManualNeeded)
2. **Decide**: (Outcome, FailPolicy) → `Verdict` — pure function, 6 rules:

```
Success + any           → Advance
ManualNeeded + any      → Yield("verify_manual")
Failure + Terminal      → Fail
Failure + Retry(can)    → Retry
Failure + Retry(!can)   → Fail
Failure + Manual        → Yield("on_fail_manual")
```

3. **Split**: emit StepFinished event, then route by Verdict (advance cursor / yield / reset step / terminate).

`can_retry` = `count_auto_retries() < max_retries`. Count scans events backward to last TaskStarted/manual StepReset.

## Dev & Test

```bash
cargo build && cargo install --path .    # Build + install
cargo test                               # Unit tests (decide() logic)
```

E2E tests (require tmux):

| Script | Tests | What | Cost |
|--------|-------|------|------|
| `tests/e2e.sh` | 136 | Sync workflows | Free |
| `tests/e2e-viewport.sh` | 27 | Viewport lifecycle | Free |
| `tests/e2e-agent.sh` | 9 | Real haiku agents | ~$0.05 |

## Session Start

1. Read `.claude/HANDOFF.md` — 上次 session 做了什么、pending 问题、key file index
2. Read `MEMORY.md` — 实战教训和陷阱（viewport 时序、pipe 退出码、测试踩坑等）
3. `git status` + `git log --oneline -5` — 当前状态

## Common Traps

- **类比推理**：不要因为 A 做了 X 就假设 B 也该做 X。先问 A **为什么**做 X，再看 B 是否有相同的 why。
- **职责越界**：每个机制只在自己的 scope 内操作。当你想在 A 里加 B 的逻辑时，问"这是 A 的职责吗？"
