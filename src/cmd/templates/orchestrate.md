# Orchestrator — Designing Workflow Config

## Top-Level Options

All optional: `session` (tmux session name, default: dir name), `viewport` (default: `"tmux"`), `worktree_dir` (default: `".pawl/worktrees"`), `base_branch` (default: `"main"`).

## Step Properties

| Property | Values | Default |
|---|---|---|
| `name` | unique identifier | (required) |
| `run` | shell command; omit → gate step (pauses for `pawl done`) | — |
| `in_viewport` | run in viewport window | `false` |
| `verify` | `"manual"` or shell command (exit 0 = pass) | — |
| `on_fail` | `"retry"` or `"manual"` | — |
| `max_retries` | retry limit when on_fail=retry | `3` |

Rules: `in_viewport` run MUST `cd ${worktree} && ...`. Failable `in_viewport` → add `on_fail` (otherwise terminal). Observable output → add `verify` (otherwise `pawl done` trusts blindly). Gate step (no `run`) → `verify`/`on_fail` ignored.

## Variables

Available as `${var}` in config commands, `PAWL_*` env vars in subprocesses:

`task` `branch` `worktree` `session` `repo_root` `step` `step_index` `base_branch` `log_file` `task_file` `run_id` `retry_count` `last_verify_output`

## Verify Strategy

| Scenario | verify | on_fail | Rationale |
|----------|--------|---------|-----------|
| Has automated tests | `"cd ${worktree} && npm test"` | `"retry"` | Fast feedback, auto-fix |
| Critical path needs manual oversight | `"manual"` | `"manual"` | Manual review + manual decision |
| Reliable tests but failure needs analysis | `"cd ${worktree} && cargo test"` | `"manual"` | Auto-detect, manual decision |
| Simple step without tests | omit | omit | Failure is terminal, manual reset |

## Event Hooks

Top-level `"on"` field maps event type → shell command (fire-and-forget, async, silent on failure).

Event types: `task_started`, `step_finished` (+`${success}` `${exit_code}` `${duration}`), `step_yielded` (+`${reason}`), `step_resumed`, `viewport_launched`, `step_skipped`, `step_reset` (+`${auto}`), `viewport_lost`, `task_stopped`, `task_reset`.

```jsonc
// Write to log file
"on": { "step_finished": "echo '[${task}] ${step} exit=${exit_code}' >> ${repo_root}/.pawl/hook.log" }

// Notify a supervisor via tmux (concurrency-safe)
"on": { "step_finished": "mkdir /tmp/pawl-notify.lock 2>/dev/null && tmux send-keys -t ${session}:supervisor -l '[pawl] ${task}/${step} finished (exit=${exit_code})' && tmux send-keys -t ${session}:supervisor C-Enter && sleep 0.3 && rmdir /tmp/pawl-notify.lock; true" }
```

## Config Recipes

### Git Worktree Skeleton

Replace `⟨work⟩` with work steps below. Omit `review` gate if work step already has `"verify": "manual"`.

```jsonc
{
  "workflow": [
    { "name": "setup",   "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    ⟨work step(s)⟩,
    { "name": "review" },
    { "name": "merge",   "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge'" },
    { "name": "cleanup", "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true" }
  ]
}
```

Multi-task: `pawl start task-a && pawl start task-b` — each gets independent JSONL/worktree/viewport. Non-git: replace setup/merge/cleanup with your own init/teardown.

### Work Steps: 2 Dimensions

All work steps start with `"run": "cd ${worktree} && <command>"`. Two orthogonal choices:

| | auto verify | manual verify |
|---|---|---|
| **viewport** | `"in_viewport": true, "verify": "<test>", "on_fail": "retry"` | `"in_viewport": true, "verify": "manual", "on_fail": "manual"` |
| **sync** | `"on_fail": "retry"` | `"verify": "manual"` |

### Retry Feedback Loop

On retry, `$PAWL_RETRY_COUNT` and `$PAWL_LAST_VERIFY_OUTPUT` are automatically available:

```bash
# In step run command:
if [ -n "$PAWL_LAST_VERIFY_OUTPUT" ]; then
  <agent-cli> "Fix: $PAWL_LAST_VERIFY_OUTPUT"
else
  <agent-cli> "$(cat $PAWL_TASK_FILE)"
fi
# Session resume: claude -r $PAWL_RUN_ID (run_id is stable across retries within a run)
```

### AI Worker Pattern

Any non-interactive CLI works. pawl provides `$PAWL_TASK_FILE` (prompt) and `$PAWL_LOG_FILE` (feedback):

```jsonc
{ "name": "develop", "run": "cd ${worktree} && cat $PAWL_TASK_FILE | <agent-cli>",
  "in_viewport": true, "verify": "cd ${worktree} && <test>", "on_fail": "retry" }
```

Compose with retry feedback loop for auto-fix on failure.

### Multi-Step Composition

Split work into sequential steps with different verify strategies (e.g. plan → execute):

```jsonc
{ "name": "plan",    "run": "cd ${worktree} && <agent> --plan-only",
  "in_viewport": true, "verify": "manual", "on_fail": "manual" },
{ "name": "develop", "run": "cd ${worktree} && <agent> --execute",
  "in_viewport": true, "verify": "cd ${worktree} && <test>", "on_fail": "retry" }
```

Plan rejection: `pawl reset --step` on the plan step.

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

### Instantiation: Worker with Session Resume

Compose the generic AI Worker Pattern + Retry Feedback Loop with Claude Code.
`$PAWL_RUN_ID` (UUID v4) is stable across retries within a run — use it directly as session ID:

```bash
# pawl work step run command:
cd ${worktree} && \
if [ -n "$PAWL_LAST_VERIFY_OUTPUT" ]; then
  claude -p "Fix: $PAWL_LAST_VERIFY_OUTPUT" -r $PAWL_RUN_ID
else
  cat $PAWL_TASK_FILE | claude -p --session-id $PAWL_RUN_ID
fi
```

- First run: `--session-id $PAWL_RUN_ID` starts a session keyed to this pawl run
- Retry: `-r $PAWL_RUN_ID` resumes session context, `$PAWL_LAST_VERIFY_OUTPUT` has failure output
- No file management needed — pawl provides the stable handle

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
