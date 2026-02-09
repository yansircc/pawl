# Orchestrator — Designing Workflow Config

## Top-Level Options

All optional: `session` (tmux session name, default: dir name), `viewport` (default: `"tmux"`), `vars` (user-defined variables, expanded in definition order).

## User Variables (`vars`)

Define project-specific variables that are expanded as `${var}` in commands and available as `PAWL_*` env vars. Later vars can reference earlier vars and intrinsic vars:

```jsonc
"vars": {
  "base_branch": "main",
  "branch": "pawl/${task}",
  "worktree": "${project_root}/.pawl/worktrees/${task}"
}
```

Two-layer model: `${var}` expanded by pawl (static, visible in logs), `$ENV_VAR` expanded by shell (dynamic, computed at runtime).

Secrets from `.env` files belong in the shell layer — use a vars prefix to avoid repetition:

```jsonc
"vars": { "env": "set -a && source ${project_root}/.env.local && set +a" }
// then in steps: "run": "${env} && npm run build"
```

## Step Properties

| Property | Values | Default |
|---|---|---|
| `name` | unique identifier | (required) |
| `run` | shell command; omit → gate step (pauses for `pawl done`) | — |
| `in_viewport` | run in viewport window | `false` |
| `verify` | `"manual"` or shell command (exit 0 = pass) | — |
| `on_fail` | `"retry"` or `"manual"` | — |
| `max_retries` | retry limit when on_fail=retry | `3` |

Rules: Failable `in_viewport` → add `on_fail` (otherwise terminal). Observable output → add `verify` (otherwise `pawl done` trusts blindly). Gate step (no `run`) → `verify`/`on_fail` ignored.

**When to add a gate step**: Only add a gate (step with no `run`) when verify is insufficient and human judgment is needed (e.g., code review, design approval). If `verify` already covers the acceptance criteria, the task completes automatically after verify passes — no gate needed. Unnecessary gates force manual `pawl done` on every task, adding overhead without value.

## Intrinsic Variables

Available as `${var}` in config commands, `PAWL_*` env vars in subprocesses:

`task` `session` `project_root` `step` `step_index` `log_file` `task_file` `run_id` `retry_count` `last_verify_output`

Plus all user vars from `config.vars`.

## Verify Strategy

| Scenario | verify | on_fail | Rationale |
|----------|--------|---------|-----------|
| Has automated tests | `"cd ${worktree} && npm test"` | `"retry"` | Fast feedback, auto-fix |
| Critical path needs manual oversight | `"manual"` | `"manual"` | Manual review + manual decision |
| Reliable tests but failure needs analysis | `"cd ${worktree} && cargo test"` | `"manual"` | Auto-detect, manual decision |
| Simple step without tests | omit | omit | Failure is terminal, manual reset |

## Event Hooks

Top-level `"on"` field maps event type → shell command (fire-and-forget, async, silent on failure).

Event types: `task_started`, `step_finished` (+`${success}` `${exit_code}` `${duration}`), `step_yielded` (+`${reason}`), `step_resumed` (+`${message}`), `viewport_launched`, `step_skipped`, `step_reset` (+`${auto}`), `viewport_lost` (safety net — only fires when `_run` crashed; normal viewport kill → `step_finished(exit_code=128)`), `task_stopped`, `task_reset`.

```jsonc
// Write to log file
"on": { "step_finished": "echo '[${task}] ${step} exit=${exit_code}' >> ${project_root}/.pawl/hook.log" }

// Notify a supervisor via tmux (concurrency-safe)
"on": { "step_finished": "mkdir /tmp/pawl-notify.lock 2>/dev/null && tmux send-keys -t ${session}:supervisor -l '[pawl] ${task}/${step} finished (exit=${exit_code})' && tmux send-keys -t ${session}:supervisor C-Enter && sleep 0.3 && rmdir /tmp/pawl-notify.lock; true" }
```

## Config Recipes

### Plain Workflow

The simplest config — steps run in project root. No vars, no git, no isolation.

```jsonc
{
  "workflow": [
    { "name": "build",  "run": "npm run build", "on_fail": "retry" },
    { "name": "review" },
    { "name": "deploy", "run": "npm run deploy" }
  ]
}
```

Add `vars` when paths repeat, `in_viewport` for long-running commands, `verify` for automated checks. Each is orthogonal — compose as needed.

### Work Steps: 2 Dimensions

Two orthogonal choices:

| | auto verify | manual verify |
|---|---|---|
| **viewport** | `"in_viewport": true, "verify": "<test>", "on_fail": "retry"` | `"in_viewport": true, "verify": "manual", "on_fail": "manual"` |
| **sync** | `"on_fail": "retry"` | `"verify": "manual"` |

### Agent Driver

A driver (adapter) is a shell script that bridges pawl with an agent CLI. Two operations:

- **start**: Launch agent. Auto-detects mode: TUI when stdin is a terminal, pipe when not.
- **read**: Read agent session logs (foreman calls this to inspect agent output).

Other operations use the substrate directly: `tmux send-keys` to send input, `pawl stop` to terminate, viewport kill to signal the agent.

Two config styles — pipe mode feeds prompt via stdin, TUI mode passes prompt as argument:

```jsonc
// Pipe mode: agent reads stdin, exits when done → _run settles
{ "name": "develop", "run": "cat $PAWL_TASK_FILE | .pawl/drivers/my-agent.sh",
  "in_viewport": true, "verify": "<test>", "on_fail": "retry" }

// TUI mode: agent runs interactively, agent /exit or foreman `pawl done` completes
{ "name": "develop", "run": ".pawl/drivers/my-agent.sh",
  "in_viewport": true, "verify": "<test>", "on_fail": "retry" }
```

```bash
#!/usr/bin/env bash
# .pawl/drivers/my-agent.sh — adapter (start + read)
set -euo pipefail
case "${1:-start}" in
  start)
    FLAGS=()
    [ -t 0 ] || FLAGS+=(<pipe-mode-flag>)
    if [ "${PAWL_RETRY_COUNT:-0}" -gt 0 ]; then
      <agent-cli> "${FLAGS[@]}" "Fix: ${PAWL_LAST_VERIFY_OUTPUT:-verify failed}"
    else
      <agent-cli> "${FLAGS[@]}"
    fi ;;
  read) <agent-log-command> ;;
esac
```

Completion detection: pipe mode → agent process exits → `_run` catches child exit → `step_finished`. TUI mode → agent `/exit` terminates process (same path), or foreman calls `pawl done`. See `references/claude-driver.sh` for a ready-to-use Claude Code adapter.

### Retry Feedback Loop

On retry, `$PAWL_RETRY_COUNT` and `$PAWL_LAST_VERIFY_OUTPUT` are automatically available. Use `PAWL_RETRY_COUNT` to detect retries (more reliable than checking if verify output is non-empty):

```bash
# In driver script:
if [ "${PAWL_RETRY_COUNT:-0}" -gt 0 ]; then
  <agent-cli> "Fix: ${PAWL_LAST_VERIFY_OUTPUT:-verify failed}"
else
  <agent-cli>  # first run
fi
# Session resume: use $PAWL_RUN_ID (stable across retries within a run)
```

### Multi-Step Composition

Split work into sequential steps with different verify strategies (e.g. plan → execute):

```jsonc
{ "name": "plan",    "run": "<agent> --plan-only",
  "in_viewport": true, "verify": "manual", "on_fail": "manual" },
{ "name": "develop", "run": "<agent> --execute",
  "in_viewport": true, "verify": "<test>", "on_fail": "retry" }
```

Plan rejection: `pawl reset --step` on the plan step.

### Git Worktree Skeleton

For git-based projects needing task isolation via worktrees. Define git vars in `config.vars`, then use them in workflow steps. Replace `⟨work⟩` with work steps above. Omit `review` gate if work step already has `"verify": "manual"`.

```jsonc
{
  "vars": {
    "base_branch": "main",
    "branch": "pawl/${task}",
    "worktree": "${project_root}/.pawl/worktrees/${task}"
  },
  "workflow": [
    { "name": "setup",   "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    ⟨work step(s)⟩,
    { "name": "review" },
    { "name": "merge",   "run": "cd ${project_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge'" },
    { "name": "cleanup", "run": "git -C ${project_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${project_root} branch -D ${branch} 2>/dev/null; true" }
  ]
}
```

Multi-task: `pawl start task-a && pawl start task-b` — each gets independent JSONL/worktree/viewport. Non-git: omit `vars` and setup/merge/cleanup; use your own init/teardown.
