# pawl

A resumable step sequencer. Define a pipeline once, run it for any task. Each task gets its own git worktree, its own viewport, and a cursor that survives crashes.

```
               ┌─ setup ─── develop ─── verify ─── merge ─── cleanup
  pawl start A ──┤
  pawl start B ──┤  Each task: own worktree, own viewport,
  pawl start C ──┘  own append-only event log.
```

## Design

Three ideas, everything else follows:

1. **`state = replay(log)`** — No database, no status file. JSONL is the single source of truth. Crash, reboot, replay, resume.
2. **Separate what from where** — Recording (what happened) and routing (what to do next) never mix. The decision function is pure: `decide(Outcome, FailPolicy) → Verdict`.
3. **Trust the substrate** — File system, exit codes, `grep`, tmux. Build only what Unix can't: coroutine semantics, failure routing, viewport abstraction.

## Install

```bash
cargo install pawl
```

Requires: Rust, tmux, git.

## Quick Start

```bash
pawl init                    # scaffold .pawl/
pawl create auth-login       # create a task
vim .pawl/tasks/auth-login.md
pawl start auth-login        # run the pipeline
```

## How It Works

Define a workflow in `.pawl/config.jsonc`:

```jsonc
{
  "workflow": [
    { "name": "setup",   "run": "git worktree add ${worktree} -b ${branch} ${base_branch}" },
    { "name": "work",    "run": "cd ${worktree} && ./run-worker.sh",
      "in_viewport": true, "verify": "cd ${worktree} && npm test", "on_fail": "retry" },
    { "name": "review" },
    { "name": "merge",   "run": "cd ${repo_root} && git merge --squash ${branch}" },
    { "name": "cleanup", "run": "git worktree remove ${worktree} --force; true" }
  ]
}
```

### Step Types

| Pattern | What happens |
|---------|-------------|
| `{ "run": "..." }` | Run sync. Non-zero exit = failure. |
| `{ "name": "..." }` | Gate — pause until `pawl done`. |
| `{ "run": "...", "in_viewport": true }` | Run in viewport, wait for `pawl done`. |
| `{ "verify": "human" }` | Run, then wait for human approval. |
| `{ "on_fail": "retry" }` | Auto-retry on failure (default: 3×). |
| `{ "on_fail": "human" }` | Yield to human on failure. |

### Variables

`${var}` in config → expanded before execution. Also available as `PAWL_VAR` env vars.

| Variable | Value |
|----------|-------|
| `${task}` | Task name |
| `${branch}` | `pawl/{task}` |
| `${worktree}` | `{repo_root}/.pawl/worktrees/{task}` |
| `${step}` | Current step name |
| `${step_index}` | Current step index (0-based) |
| `${repo_root}` | Git repository root |
| `${base_branch}` | Config base branch |
| `${session}` | Viewport session name |
| `${task_file}` | `.pawl/tasks/{task}.md` |
| `${log_file}` | `.pawl/logs/{task}.jsonl` |
| `${run_id}` | UUID v4 for current run |
| `${retry_count}` | Auto-retry count for current step |
| `${last_verify_output}` | Last failure output |

## Output

stdout = JSON (write commands) or JSONL (log/events). stderr = progress. Pipe to `jq` for human reading.

```bash
pawl status task-a | jq .          # pretty-print JSON
pawl log task-a --all | jq .       # pretty-print JSONL events
pawl start task-a 2>/dev/null      # JSON only, no progress
```

## Commands

```bash
# Lifecycle
pawl init                          # Initialize project
pawl create <name>                 # Create task
pawl start <task> [--reset]        # Start pipeline

# Flow control
pawl done <task> [-m msg]          # Approve / mark done
pawl stop <task>                   # Stop
pawl reset <task>                  # Reset task
pawl reset --step <task>           # Retry current step

# Observe
pawl status [task]                 # Status (JSON)
pawl list                          # List tasks (JSON array)
pawl log <task> [--all] [--step N] # Logs (JSONL)
pawl events [task] [--follow]      # Event stream (JSONL)
pawl capture <task> [-l N]         # Viewport content (JSON)
pawl wait <task> --until <status>  # Poll (exit code semantic)
pawl enter <task>                  # Attach to viewport
```

## Task Files

`.pawl/tasks/{name}.md` — YAML frontmatter + markdown body:

```markdown
---
name: auth-login
depends:
  - database-setup
skip:
  - cleanup
---

Implement login with email/password. Return JWT. Rate limit 5/min.
```

## Project Layout

```
.pawl/
├── config.jsonc              # Workflow definition (self-documented)
├── tasks/*.md                # Task specs
├── logs/*.jsonl              # Event logs (single source of truth)
├── skills/pawl/              # Skill reference
│   ├── SKILL.md              # Orientation + role routing
│   └── references/           # Role-specific guides
│       ├── author.md         # Writing tasks
│       ├── orchestrate.md    # Designing workflows
│       └── supervise.md      # Monitoring tasks
└── worktrees/*/              # Git worktrees (one per task)
```

State = `replay(logs/*.jsonl)`. No status file.

## License

MIT
