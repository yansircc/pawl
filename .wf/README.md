# .wf/ Directory — Configuration Guide

This README helps you write a `config.jsonc` workflow configuration for your project.

## Quick Start

```jsonc
{
  "base_branch": "main",
  "workflow": [
    { "name": "Create branch", "run": "git branch ${branch} ${base_branch}" },
    { "name": "Create worktree", "run": "git worktree add ${worktree} ${branch}" },
    { "name": "Create window", "run": "tmux new-window -t ${session} -n ${window} -c ${worktree}" },
    {
      "name": "Develop",
      "run": "claude -p '@${task_file}'",
      "in_window": true
    },
    { "name": "Merge", "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): done'" },
    { "name": "Cleanup", "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true" }
  ]
}
```

## Config Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `session` | string? | project dir name | tmux session name |
| `worktree_dir` | string? | `".wf/worktrees"` | worktree directory (relative to repo root) |
| `base_branch` | string? | `"main"` | base branch for creating task branches |
| `workflow` | Step[] | **required** | ordered list of steps |
| `hooks` | object? | `{}` | event hooks (event name -> shell command) |

### Step Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | **required** | step display name |
| `run` | string? | _(omit for checkpoint)_ | shell command to execute |
| `in_window` | bool? | `false` | run in tmux window (agent mode) |
| `stop_hook` | string? | — | validation command; must exit 0 for `wf done` to succeed |

## Variables

All variables are expanded in `run` and `stop_hook` fields. They are also set as environment variables (`WF_*`) in subprocesses.

| Variable | Env Var | Example Value |
|----------|---------|---------------|
| `${task}` | `WF_TASK` | `auth` |
| `${branch}` | `WF_BRANCH` | `wf/auth` |
| `${worktree}` | `WF_WORKTREE` | `/home/project/.wf/worktrees/auth` |
| `${window}` | `WF_WINDOW` | `auth` |
| `${session}` | `WF_SESSION` | `my-project` |
| `${repo_root}` | `WF_REPO_ROOT` | `/home/project` |
| `${step}` | `WF_STEP` | `Type check` |
| `${base_branch}` | `WF_BASE_BRANCH` | `main` |
| `${log_file}` | `WF_LOG_FILE` | `/home/project/.wf/logs/auth.jsonl` |
| `${task_file}` | `WF_TASK_FILE` | `/home/project/.wf/tasks/auth.md` |
| `${step_index}` | `WF_STEP_INDEX` | `7` |

**Key notes:**
- `${branch}` is always `wf/{task}`, auto-generated
- `${worktree}` is always `{repo_root}/{worktree_dir}/{task}`, auto-generated
- `${window}` equals `${task}`
- `${log_file}`, `${task_file}`, `${step_index}` are absolute paths/values, available during execution

## Step Types

### Normal Step

Runs synchronously. If exit code is 0, proceeds to next step. Otherwise, task fails.

```jsonc
{ "name": "Install deps", "run": "cd ${worktree} && npm install" }
```

### Checkpoint

Omit the `run` field. Workflow pauses until a human runs `wf next <task>`.

```jsonc
{ "name": "Review changes" }
```

### in_window Step

Runs in a tmux window. The workflow pauses until the agent calls `wf done/fail/block`.

```jsonc
{
  "name": "Develop",
  "run": "claude -p '@${task_file}'",
  "in_window": true
}
```

## Directory Structure

```
.wf/
├── config.jsonc      # This file — workflow configuration
├── status.json       # Runtime state (managed by wf, do not edit)
├── tasks/            # Task definitions
│   └── {name}.md     # Markdown with optional YAML frontmatter
├── logs/             # Execution logs
│   └── {name}.jsonl  # One JSONL file per task
├── worktrees/        # Git worktrees (managed by workflow)
│   └── {name}/
└── hooks/            # Hook configs (e.g. Claude settings.json)
```

## Best Practices

### 1. Workflow Phase Design

Organize steps into clear phases:

```jsonc
{
  "workflow": [
    // === Setup ===
    { "name": "Create branch", "run": "git branch ${branch} ${base_branch}" },
    { "name": "Create worktree", "run": "git worktree add ${worktree} ${branch}" },
    { "name": "Create window", "run": "tmux new-window -t ${session} -n ${window} -c ${worktree}" },
    { "name": "Copy .env", "run": "cp ${repo_root}/.env ${worktree}/.env" },
    { "name": "Install deps", "run": "cd ${worktree} && npm install" },

    // === Development ===
    { "name": "Develop", "run": "claude -p '@${task_file}'", "in_window": true },

    // === Verification ===
    { "name": "Type check", "run": "cd ${worktree} && npm run typecheck" },
    { "name": "Lint", "run": "cd ${worktree} && npm run lint" },
    { "name": "Build", "run": "cd ${worktree} && npm run build" },

    // === Merge & Cleanup ===
    { "name": "Merge", "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): done'" },
    { "name": "Cleanup", "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true" }
  ]
}
```

### 2. Passing Context to AI Agents

Use `${task_file}` to pass the task definition to the agent:

```jsonc
// Good: use absolute path variable
{ "run": "claude -p '@${task_file}'" }

// Also good: use repo_root for explicit paths
{ "run": "claude -p '@${repo_root}/.wf/tasks/${task}.md'" }
```

### 3. Passing Log History to Later Steps

Use `${log_file}` so later steps can read previous execution results:

```jsonc
// Code review step can read task logs for context
{
  "name": "Code review",
  "run": "claude --tools 'Read,Grep,Glob' -p 'Review changes. Logs: @${log_file} Task: @${task_file}'",
  "in_window": true
}
```

### 4. Saving Artifacts for Later Steps

Write intermediate outputs to the logs directory, then reference them:

```jsonc
// Save a diff for review
{
  "name": "Save diff",
  "run": "cd ${worktree} && git diff HEAD --stat > ${repo_root}/.wf/logs/${task}-diff.txt && git diff HEAD >> ${repo_root}/.wf/logs/${task}-diff.txt"
},
// Review step reads the saved diff
{
  "name": "Code review",
  "run": "claude -p 'Review @${repo_root}/.wf/logs/${task}-diff.txt against @${task_file}'",
  "in_window": true
}
```

### 5. Scoping Agent Permissions

Limit agent tools to reduce risk and save tokens:

```jsonc
// Full access for development
{ "name": "Develop", "run": "claude -p '@${task_file}'", "in_window": true },

// Read-only for review
{ "name": "Review", "run": "claude --tools 'Read,Grep,Glob' -p 'Review the code'", "in_window": true },

// Bash-only for commit
{ "name": "Commit", "run": "claude --tools 'Bash,Read' -p 'Commit changes'", "in_window": true }
```

### 6. Using Custom Claude Settings

Use `--settings` to override Claude config per step:

```jsonc
{
  "name": "Develop",
  "run": "claude --settings ${repo_root}/.wf/hooks/settings.json --setting-sources '' --disable-slash-commands -p '@${task_file}'",
  "in_window": true
}
```

### 7. Cleanup Step

Always make cleanup idempotent and non-failing:

```jsonc
{
  "name": "Cleanup",
  "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true"
}
```

**Important:** Do NOT `kill-window` in cleanup — the `_on-exit` handler is still running in that window. The window is cleaned up automatically by `wf done`.

### 8. Creating Branch from Base

Always specify `${base_branch}` when creating a branch:

```jsonc
// Good: explicit base
{ "run": "git branch ${branch} ${base_branch}" }

// Bad: implicit HEAD (may not be what you want)
{ "run": "git branch ${branch}" }
```

### 9. Stop Hook for Quality Gates

Add validation before allowing `wf done`:

```jsonc
{
  "name": "Develop",
  "run": "claude -p '@${task_file}'",
  "in_window": true,
  "stop_hook": "cd ${worktree} && npm run typecheck && npm run lint"
}
```

### 10. Working in the Worktree

Always `cd ${worktree}` for commands that need to run in the task's worktree:

```jsonc
// Good
{ "run": "cd ${worktree} && npm test" }

// Bad: runs in repo root
{ "run": "npm test" }
```

## Task File Format

Task files live in `.wf/tasks/{name}.md`:

```markdown
---
name: auth-login
depends:
  - database-setup
---

## Task Description

Implement the login endpoint...
```

The `depends` field is optional. If specified, `wf start` will check that dependencies are completed first.

## Complete Example (Next.js Project)

```jsonc
{
  "base_branch": "main",

  "workflow": [
    // Setup
    { "name": "Create branch", "run": "git branch ${branch} ${base_branch}" },
    { "name": "Create worktree", "run": "git worktree add ${worktree} ${branch}" },
    { "name": "Create window", "run": "tmux new-window -t ${session} -n ${window} -c ${worktree}" },
    { "name": "Copy .env", "run": "cp ${repo_root}/.env ${worktree}/.env" },
    { "name": "Install deps", "run": "cd ${worktree} && bun i" },
    { "name": "DB generate", "run": "cd ${worktree} && bun db:generate" },
    { "name": "DB push", "run": "cd ${worktree} && bun db:push" },

    // Development
    {
      "name": "Develop",
      "run": "claude --settings ${repo_root}/.wf/hooks/settings.json --setting-sources '' --disable-slash-commands -p '@${task_file}'",
      "in_window": true
    },

    // Verification
    { "name": "Type check", "run": "cd ${worktree} && bun typecheck" },
    { "name": "Lint check", "run": "cd ${worktree} && bun check" },

    // Review
    {
      "name": "Save diff",
      "run": "cd ${worktree} && git diff HEAD~0 --stat > ${repo_root}/.wf/logs/${task}-diff.txt && git diff HEAD~0 >> ${repo_root}/.wf/logs/${task}-diff.txt"
    },
    {
      "name": "Code review",
      "run": "claude --settings ${repo_root}/.wf/hooks/settings.json --setting-sources '' --disable-slash-commands --tools 'Read,Grep,Glob' -p 'Review @${repo_root}/.wf/logs/${task}-diff.txt against @${task_file}'",
      "in_window": true
    },

    // Build
    { "name": "Build", "run": "cd ${worktree} && bun run build" },

    // Commit & Merge
    {
      "name": "Commit & Merge",
      "run": "claude --settings ${repo_root}/.wf/hooks/settings.json --setting-sources '' --disable-slash-commands --tools 'Bash,Read' -p 'In ${worktree}, commit all changes. Then in ${repo_root}, git merge --squash ${branch} and commit.'",
      "in_window": true
    },

    // Cleanup
    {
      "name": "Cleanup",
      "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true"
    }
  ]
}
```
