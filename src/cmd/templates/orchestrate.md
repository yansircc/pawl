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

Event types: `task_started`, `step_finished` (+`${success}` `${exit_code}` `${duration}`), `step_yielded` (+`${reason}`), `step_resumed`, `viewport_launched`, `step_skipped`, `step_reset` (+`${auto}`), `viewport_lost` (safety net — only fires when `_run` crashed; normal viewport kill → `step_finished(exit_code=128)`), `task_stopped`, `task_reset`.

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

A driver is a shell script that bridges pawl with an agent CLI. Four operations:

- **start**: Launch agent. Prompt via stdin, retry feedback via `$PAWL_LAST_VERIFY_OUTPUT`.
- **send**: Send instruction to running agent (tmux send-keys).
- **stop**: Terminate running agent.
- **read**: Read agent output/logs.

```jsonc
{ "name": "develop", "run": "cat $PAWL_TASK_FILE | .pawl/drivers/my-agent.sh start",
  "in_viewport": true, "verify": "<test>", "on_fail": "retry" }
```

```bash
#!/usr/bin/env bash
# .pawl/drivers/my-agent.sh — start/send/stop/read
set -euo pipefail
case "${1:?Usage: $0 start|send|stop|read}" in
  start)
    if [ "${PAWL_RETRY_COUNT:-0}" -gt 0 ]; then
      <agent-cli> "Fix: ${PAWL_LAST_VERIFY_OUTPUT:-verify failed}"
    else
      <agent-cli>  # reads prompt from stdin
    fi ;;
  send)  shift; tmux send-keys -t "$PAWL_SESSION:$PAWL_TASK" "$*" Enter ;;
  stop)  tmux send-keys -t "$PAWL_SESSION:$PAWL_TASK" C-c ;;
  read)  <agent-log-command> ;;
esac
```

Prompt flows via stdin — compose in the `run` command: `cat $PAWL_TASK_FILE | driver start`, `echo "custom" | driver start`, or pipe any generator. See `references/claude-driver.sh` for a ready-to-use Claude Code driver with session resume.

### Retry Feedback Loop

On retry, `$PAWL_RETRY_COUNT` and `$PAWL_LAST_VERIFY_OUTPUT` are automatically available. Use `PAWL_RETRY_COUNT` to detect retries (more reliable than checking if verify output is non-empty):

```bash
# In driver script:
if [ "${PAWL_RETRY_COUNT:-0}" -gt 0 ]; then
  <agent-cli> "Fix: ${PAWL_LAST_VERIFY_OUTPUT:-verify failed}"
else
  <agent-cli>  # first run
fi
# Session resume: claude -r $PAWL_RUN_ID (run_id is stable across retries within a run)
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

## Claude Code CLI for Workflows

Workflow-essential flags (full reference: `claude --help`):

| Flag | Purpose |
|------|---------|
| `-p` | Non-interactive mode (**required** for pawl workers) |
| `--output-format stream-json` | **Recommended output format**. Streaming JSON events (requires `--verbose`) |
| `--input-format stream-json` | Streaming JSON input (programmatic multi-turn, `-p` only) |
| `-r <session_id>` | Resume session (full context preserved across retries) |
| `--session-id <uuid>` | Specify session ID (must be valid UUID, enables deterministic resume) |
| `--permission-mode plan` | Plan-only mode (reviews before execution) |
| `--dangerously-skip-permissions` | **Default for automation**. Skip all permission prompts (otherwise worker blocks) |
| `--append-system-prompt "..."` | Inject extra instructions (preserves defaults) |
| `--append-system-prompt-file path` | Same, from file (version-controllable) |
| `--json-schema '{...}'` | Force structured JSON output (validated against schema) |
| `--tools "Bash,Read,Edit"` | Restrict available tools (empty `""` = no tools) |

### Instantiation: Claude Code Driver

Copy `references/claude-driver.sh` to `.pawl/drivers/claude.sh`. The driver uses `$PAWL_RUN_ID` as session ID — first run via `--session-id`, retries via `-r` (full context preserved).

```jsonc
{ "name": "develop", "run": "cat $PAWL_TASK_FILE | .pawl/drivers/claude.sh start",
  "in_viewport": true, "verify": "<test>", "on_fail": "retry" }
```

Reduce system prompt tokens (~13x) with:

```bash
--tools "Bash,Write" --setting-sources "" --mcp-config '{"mcpServers":{}}' --disable-slash-commands
```

### Instantiation: Plan-Then-Execute

```jsonc
{ "name": "plan",    "run": "cd ${worktree} && cat ${task_file} | claude -p --session-id $PAWL_RUN_ID --permission-mode plan",
  "in_viewport": true, "verify": "manual", "on_fail": "manual" },
{ "name": "develop", "run": "cd ${worktree} && claude -p 'Execute the approved plan.' -r $PAWL_RUN_ID --dangerously-skip-permissions",
  "in_viewport": true, "verify": "cd ${worktree} && <test>", "on_fail": "retry" }
```

### Structured Output for Decisions

Use `--json-schema` when a supervisor needs machine-readable decisions:

```bash
claude -p "Analyze this error and decide: retry or escalate?" \
  --output-format stream-json --verbose \
  --json-schema '{"type":"object","properties":{"action":{"enum":["retry","escalate"]},"reason":{"type":"string"}},"required":["action","reason"]}' \
  | grep '"type":"result"' | jq '.structured_output'
```
