# wf

An orchestrator for AI coding agents. Each agent gets its own git worktree, its own tmux window, and a configurable pipeline — from branch creation to merge. You define the pipeline once, then launch as many agents as you want.

```
               ┌─ setup ─── develop ─── verify ─── merge ─── cleanup
  wf start A ──┤
  wf start B ──┤  Each task runs in its own worktree,
  wf start C ──┘  isolated from the others.
```

## Why

AI coding agents (Claude, Codex, etc.) are powerful but messy to run in parallel. They conflict on files, break each other's imports, and leave merge chaos behind. Manual worktree/branch/merge management doesn't scale past 2-3 agents.

`wf` gives each agent an isolated workspace and a structured lifecycle: setup, develop, verify, merge, clean up. The agent communicates back via a simple protocol (`wf done / wf fail / wf block`). Everything in the pipeline is a shell command — no plugins, no SDKs.

## Install

```bash
cargo install --path .
```

Requires: Rust toolchain, tmux, git.

## Quick Start

```bash
# 1. Initialize in your project
cd your-project
wf init

# 2. Create a task
wf create auth-login

# 3. Write the task spec
vim .wf/tasks/auth-login.md

# 4. Start the task
wf start auth-login

# 5. Monitor with TUI
wf tui
```

## How It Works

You define a workflow in `.wf/config.jsonc` — a list of steps that run for every task:

```jsonc
{
  "base_branch": "main",
  "workflow": [
    { "name": "Create branch", "run": "git branch ${branch} ${base_branch}" },
    { "name": "Create worktree", "run": "git worktree add ${worktree} ${branch}" },
    { "name": "Create window", "run": "tmux new-window -t ${session} -n ${window} -c ${worktree}" },
    {
      "name": "Develop",
      "run": "claude -p '@.wf/tasks/${task}.md'",
      "in_window": true
    },
    { "name": "Type check", "run": "cd ${worktree} && npm run typecheck" },
    { "name": "Merge", "run": "cd ${repo_root} && git merge --squash ${branch}" },
    { "name": "Cleanup", "run": "git worktree remove ${worktree} --force; git branch -D ${branch}; true" }
  ]
}
```

### Step Types

| Type | Config | Behavior |
|------|--------|----------|
| **Command** | `{ "name": "...", "run": "..." }` | Runs synchronously. Fails on non-zero exit. |
| **Checkpoint** | `{ "name": "..." }` | Pauses until you run `wf next`. |
| **Agent** | `{ "run": "...", "in_window": true }` | Runs in tmux. Waits for `wf done/fail/block`. |

### Variables

All `${var}` references are expanded before execution:

| Variable | Example |
|----------|---------|
| `${task}` | `auth-login` |
| `${branch}` | `wf/auth-login` |
| `${worktree}` | `/project/.wf/worktrees/auth-login` |
| `${session}` | `my-project` |
| `${repo_root}` | `/project` |
| `${base_branch}` | `main` |
| `${task_file}` | `/project/.wf/tasks/auth-login.md` |
| `${log_file}` | `/project/.wf/logs/auth-login.jsonl` |

## Commands

### Lifecycle

```bash
wf init                  # Initialize .wf/ directory
wf create <name>         # Create a task
wf start <task>          # Start workflow execution
wf tui                   # Interactive dashboard
```

### Flow Control

```bash
wf next <task>           # Continue past checkpoint
wf retry <task>          # Retry failed step
wf back <task>           # Go back one step
wf skip <task>           # Skip current step
wf stop <task>           # Stop running task
wf reset <task>          # Reset to initial state
```

### Monitoring

```bash
wf status [task]         # Show status (--json for machine output)
wf list                  # List all tasks
wf log <task> --all      # View execution logs
wf log <task> --step 3   # View specific step log
wf capture <task>        # Capture tmux window content
```

### Agent Commands

These are called by AI agents running inside tmux windows:

```bash
wf done <task>           # Mark step complete (runs verify if set)
wf fail <task>           # Mark step failed
wf block <task>          # Mark step blocked (needs human help)
```

## TUI

`wf tui` opens an interactive terminal UI showing all tasks, their progress, and live tmux window content.

## Task Files

Tasks are defined as markdown in `.wf/tasks/`:

```markdown
---
name: auth-login
depends:
  - database-setup
---

Implement the login API endpoint with email/password authentication.

## Requirements
- POST /api/auth/login
- Return JWT token
- Rate limit: 5 attempts per minute
```

## Project Layout

```
.wf/
├── config.jsonc          # Workflow configuration
├── tasks/                # Task definitions (*.md)
├── logs/                 # Event logs (*.jsonl) — single source of truth
├── worktrees/            # Git worktrees (one per task)
└── hooks/                # Hook configs
```

State is reconstructed from event logs via replay — no `status.json`.

## License

MIT
