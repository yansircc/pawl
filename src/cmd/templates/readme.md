# pawl — Resumable Step Sequencer

Shell's missing `yield`. Turns any shell pipeline into a resumable coroutine with failure routing. Define a step sequence, run it for any task — pawl yields when it needs judgment, retries on failure, and rebuilds state from log after crash.

Core invariant: `state = replay(log)`. Append-only JSONL is the single source of truth.

stdout = JSON/JSONL, stderr = plain text (progress/errors). `pawl status` includes routing hints (`suggest`/`prompt`) so consumers don't need to understand pawl internals.

## Workflows

Each `.pawl/workflows/*.json` file defines an independent workflow with its own step sequence, tasks, variables, and hooks. The file name (without `.json`) becomes the workflow name.

Task names must be globally unique across all workflow files.

### Workflow File Schema

| Field | Description | Default |
|-------|-------------|---------|
| `workflow` | Step sequence (required) | — |
| `vars` | User-defined variables | — |
| `tasks` | Per-task metadata (depends, skip) | — |
| `on` | Event hooks | — |
| `session` | tmux session name | directory name |
| `viewport` | Viewport backend | `"tmux"` |

### Example: Single Workflow

`.pawl/workflows/default.json`:
```json
{
  "workflow": [
    { "name": "build",  "run": "npm run build", "on_fail": "retry" },
    { "name": "review" },
    { "name": "deploy", "run": "npm run deploy" }
  ]
}
```

### Example: Multiple Workflows

`.pawl/workflows/build.json`:
```json
{
  "workflow": [
    { "name": "compile", "run": "make build" },
    { "name": "test",    "run": "make test", "on_fail": "retry" }
  ],
  "tasks": {
    "lib":  { "description": "Core library" },
    "api":  { "depends": ["lib"] }
  }
}
```

`.pawl/workflows/deploy.json`:
```json
{
  "workflow": [
    { "name": "stage",   "run": "deploy --env staging" },
    { "name": "verify",  "verify": "manual" },
    { "name": "promote", "run": "deploy --env prod" }
  ],
  "tasks": {
    "release": { "depends": ["lib", "api"] }
  }
}
```

Dependencies can reference tasks from other workflows (e.g., `release` depends on `lib` and `api` from the build workflow).

### Step Properties

| Property | Value | Default |
|----------|-------|---------|
| `name` | Unique identifier | (required) |
| `run` | Shell command; omit → gate step | — |
| `in_viewport` | Run in viewport window | `false` |
| `verify` | `"manual"` or shell command (exit 0 = pass) | — |
| `on_fail` | `"retry"` or `"manual"` | — |
| `max_retries` | Retry limit when on_fail=retry | `3` |

Rules:
- Fallible `in_viewport` → add `on_fail` (otherwise failure is terminal)
- Observable output → add `verify` (otherwise `pawl done` trusts blindly)
- Gate step (no `run`) → `verify`/`on_fail` are ignored
- Only use gate when verify isn't enough and human judgment is needed

### Tasks (optional)

Declare inter-task dependencies and per-task step skipping. Undeclared tasks can `pawl start` freely with no constraints (only when using a single workflow):

```json
{
  "tasks": {
    "database": { "description": "Database schema and migrations" },
    "auth": { "description": "JWT auth module", "depends": ["database"] },
    "api": { "description": "REST API endpoints", "depends": ["auth", "database"], "skip": ["review"] }
  },
  "workflow": [...]
}
```

- **description**: Human-readable task description, shown in `pawl list`/`pawl status` JSON output
- **depends**: Prerequisite task list. **Enforced**: incomplete deps → `pawl start` refuses (exit 3). Can reference tasks in other workflows.
- **skip**: Step names to auto-skip for this task

All three fields are optional. With multiple workflows, all tasks must be declared in a workflow file.

### Variables

Two layers: `${var}` expanded by pawl (static, visible in logs), `$ENV_VAR` expanded by shell at runtime (dynamic).

Built-in: `task` `workflow` `session` `project_root` `step` `step_index` `log_file` `run_id` `retry_count` `last_verify_output`

User variables via `"vars"` in workflow files, expanded in declaration order. Later vars can reference earlier vars and built-in vars:

```json
{
  "vars": {
    "branch": "pawl/${task}",
    "worktree": "${project_root}/.pawl/worktrees/${task}"
  }
}
```

All variables available as `PAWL_*` env vars in subprocesses (e.g., `$PAWL_RUN_ID`).

### Event Hooks

Top-level `"on"` maps event type → shell command (fire-and-forget, async, silent on failure). All context variables are available in hook commands. Each workflow file has its own hooks:

```json
{
  "on": {
    "step_finished": "echo '[${task}] ${step} exit=${exit_code}' >> ${project_root}/.pawl/hook.log"
  }
}
```

Event types and extra variables:

| Event | Extra vars |
|-------|------------|
| `task_started` | `${run_id}` `${workflow}` |
| `step_finished` | `${success}` `${exit_code}` `${duration}` |
| `step_yielded` | `${reason}` |
| `step_resumed` | `${message}` |
| `step_reset` | `${auto}` |
| `viewport_launched` `step_skipped` `viewport_lost` `task_stopped` `task_reset` | — |

## CLI Commands

| Command | Purpose |
|---------|---------|
| `pawl init` | Initialize `.pawl/` scaffold |
| `pawl start <name> [--reset]` | Execute task (--reset: auto-reset before start) |
| `pawl status [name]` | Query status (includes suggest/prompt routing hints) |
| `pawl list` | List all task statuses |
| `pawl done <name> [-m msg]` | Approve waiting step or complete in_viewport step |
| `pawl stop <name>` | Stop a running task |
| `pawl reset <name> [--step]` | Reset task or single step |
| `pawl wait <name...> --until <status> [-t sec] [--any]` | Block until target status |
| `pawl events [name] [--follow] [--type ...]` | Event stream (live or historical) |
| `pawl log <name> [--step N] [--all]` | View log events |
| `pawl serve [--port N] [--ui file]` | HTTP API server (default: 3131) |
| `pawl _run` | Internal: viewport parent process |

**Task indexing**: tasks can be referenced by name or 1-based index (e.g., `pawl start 1` = first task).

**Conventions**: Indices are 0-based in JSON output. `suggest` = mechanical commands (execute directly), `prompt` = requires judgment. `pawl done` is always in the prompt.

### Exit Codes

| Code | Meaning | Example |
|------|---------|---------|
| 0 | Success | — |
| 1 | Internal error | IO failure, unexpected panic |
| 2 | State conflict | Task already running, invalid state transition |
| 3 | Precondition failed | Dependencies incomplete |
| 4 | Not found | Unknown task name |
| 5 | Already exists | `.pawl/` already initialized |
| 6 | Validation error | Invalid status value, unknown viewport backend |
| 7 | Timeout | `pawl wait` exceeded `-t` limit |

### Viewport Lost

If an `in_viewport` step's tmux window disappears (user closes it, tmux crash), pawl detects it on the next operation (`pawl status`, `pawl done`) and emits a `viewport_lost` event. The task transitions to Failed.

## Reference

**Task**: A named instance of a workflow. `pawl start foo` creates an independent event log for `foo`.

**Workflow**: A step sequence defined in `.pawl/workflows/<name>.json`. Each workflow file is a complete, independent configuration.

**Step types**: run (shell command), gate (no `run` — pauses for `pawl done`), viewport (`"in_viewport": true` — runs in tmux window).

**TaskStatus**: `Pending` → `Running` → `Waiting` / `Completed` / `Failed` / `Stopped`

**StepStatus**: `Success` | `Failed` | `Skipped`
