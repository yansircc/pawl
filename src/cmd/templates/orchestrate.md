# Orchestrator — Designing Workflow Config

## Verify Strategy

| Scenario | verify | on_fail | Rationale |
|----------|--------|---------|-----------|
| Has automated tests | `"cd ${worktree} && npm test"` | `"retry"` | Fast feedback, auto-fix |
| Critical path needs human oversight | `"human"` | `"human"` | Human review + human decision |
| Reliable tests but failure needs analysis | `"cd ${worktree} && cargo test"` | `"human"` | Auto-detect, human decision |
| Simple step without tests | omit | omit | Failure is terminal, manual reset |

## Event Hook Examples

```jsonc
// Write to log file
"on": { "step_finished": "echo '[${task}] ${step} exit=${exit_code}' >> ${repo_root}/.pawl/hook.log" }

// Notify a supervisor via tmux (concurrency-safe)
"on": { "step_finished": "mkdir /tmp/pawl-notify.lock 2>/dev/null && tmux send-keys -t ${session}:supervisor -l '[pawl] ${task}/${step} finished (exit=${exit_code})' && tmux send-keys -t ${session}:supervisor C-Enter && sleep 0.3 && rmdir /tmp/pawl-notify.lock; true" }
```

## Config Recipes

### Git Worktree Skeleton

Replace `⟨work⟩` with work steps below. Omit `review` gate if work step already has `"verify": "human"`.

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

| | auto verify | human verify |
|---|---|---|
| **viewport** | `"in_viewport": true, "verify": "<test>", "on_fail": "retry"` | `"in_viewport": true, "verify": "human", "on_fail": "human"` |
| **sync** | `"on_fail": "retry"` | `"verify": "human"` |

### Retry Feedback Loop

On retry, extract last failure's `verify_output` and inject into next attempt:

```bash
FB=$(grep '"step_finished"' $PAWL_LOG_FILE | grep '"success":false' | tail -1 | jq -r '.verify_output // empty' 2>/dev/null)
# Use: <agent-cli> "Fix: $FB"
# Session resume (tool-specific): claude -r $(cat .session-id), etc.
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
  "in_viewport": true, "verify": "human", "on_fail": "human" },
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
| `--permission-mode plan` | Plan-only mode (human reviews before execution) |
| `--dangerously-skip-permissions` | **Default for automation**. Skip all permission prompts (otherwise worker blocks) |
| `--append-system-prompt "..."` | Inject extra instructions (preserves defaults) |
| `--append-system-prompt-file path` | Same, from file (version-controllable) |
| `--json-schema '{...}'` | Force structured JSON output (validated against schema) |
| `--tools "Bash,Read,Edit"` | Restrict available tools (empty `""` = no tools) |

### Instantiation: Worker with Session Resume

Compose the generic AI Worker Pattern + Retry Feedback Loop with Claude Code:

```bash
# pawl work step run command:
cd ${worktree} && SF=.claude-session && \
FB=$(grep '"step_finished"' $PAWL_LOG_FILE | grep '"success":false' | tail -1 | jq -r '.verify_output // empty' 2>/dev/null) && \
if [ -f $SF ] && [ -n "$FB" ]; then
  claude -p "Fix: $FB" -r $(cat $SF)
else
  SID=$(uuidgen) && echo $SID > $SF && \
  cat $PAWL_TASK_FILE | claude -p --session-id $SID
fi
```

- First run: pre-generates UUID via `--session-id`, saves to file (no output parsing needed)
- Retry: `-r $(cat $SF)` resumes session context, injects `verify_output` as feedback
- Session file (`$SF`) is per-worktree, survives across retries

### Instantiation: Plan-Then-Execute

```jsonc
{ "name": "plan",    "run": "cd ${worktree} && SID=$(uuidgen) && echo $SID > .claude-session && cat ${task_file} | claude -p --session-id $SID --permission-mode plan",
  "in_viewport": true, "verify": "human", "on_fail": "human" },
{ "name": "develop", "run": "cd ${worktree} && claude -p 'Execute the approved plan.' -r $(cat .claude-session) --dangerously-skip-permissions",
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
