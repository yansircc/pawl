# pawl — Resumable Step Sequencer

pawl is a **resumable coroutine**: advance along a fixed step sequence, yield at decision points, rebuild state from an append-only log. Any repeatable multi-step process can be a pawl workflow — AI coding with git worktrees, testing pipelines, deployment automation, project bootstrapping. Steps support verify/retry/gate for human-in-the-loop control; viewports for long-running processes.

## CLI Commands

| Command | Description |
|---------|-------------|
| `pawl init` | Initialize project (creates .pawl/) |
| `pawl create <name> [desc] [--depends a,b]` | Create a task |
| `pawl list` | List all tasks and their status |
| `pawl start <task> [--reset]` | Start task execution (--reset resets first) |
| `pawl status [task] [--json]` | Show status (--json uses 0-based index) |
| `pawl stop <task>` | Stop a task |
| `pawl reset <task>` | Fully reset a task |
| `pawl reset --step <task>` | Retry current step |
| `pawl done <task> [-m msg]` | Approve / mark done |
| `pawl enter <task>` | Attach to viewport |
| `pawl capture <task> [-l N] [--json]` | Capture viewport content |
| `pawl wait <task> --until <status> [-t sec]` | Wait for specified status |
| `pawl log <task> [--step N] [--all] [--all-runs]` | View logs |
| `pawl events [task] [--follow]` | Raw event stream |

## Step Model

Each step has 4 orthogonal properties: `run`, `verify`, `on_fail`, `in_viewport`

| Type | Config | Behavior |
|------|--------|----------|
| Normal step | `"run": "cmd"` | Runs synchronously, exit 0 advances, otherwise Failed |
| Gate | no `run` | Pauses immediately, waits for `pawl done` |
| Human review | `"verify": "human"` | Runs, then pauses for human review |
| Auto verify | `"verify": "test.sh"` | Runs, then executes verify script (exit 0 passes) |
| Auto retry | `"on_fail": "retry"` | Auto-retries on failure (max_retries, default 3) |
| Human decision | `"on_fail": "human"` | Pauses on failure for human decision |
| Viewport task | `"in_viewport": true` | Runs in viewport, waits for `pawl done` |

### Config Design Rules

When generating or modifying `.pawl/config.jsonc`, these rules are mandatory:

1. **Every failable in_viewport step must define `on_fail`** ("retry" or "human"), otherwise failure is terminal
2. **Every step with observable output must define `verify`**, otherwise `pawl done` trusts unconditionally
3. **in_viewport step's `run` must `cd` to the working directory** (e.g. `cd ${worktree}`, `cd ~/projects/${task}`), otherwise worker runs in wrong directory

Exception: utility steps (git setup, merge, cleanup) may omit verify/on_fail when terminal failure is acceptable — the operator investigates and resets manually.

### Anti-patterns

| Config | Problem | Fix |
|--------|---------|-----|
| Gate + verify/on_fail | Gate has no run, verify/on_fail are ignored | Remove verify/on_fail, or add run |
| in_viewport without verify | `pawl done` trusts unconditionally, can't detect errors | Add `verify` |
| in_viewport with verify but no on_fail | Verify failure is terminal, can't retry | Add `on_fail` |
| in_viewport run without cd | Worker runs in repo root | `cd ${worktree} &&` or `cd /path/${task} &&` |

### Verify Strategy

| Scenario | verify | on_fail | Rationale |
|----------|--------|---------|-----------|
| Has automated tests | `"cd ${worktree} && npm test"` | `"retry"` | Fast feedback, auto-fix |
| Critical path needs human oversight | `"human"` | `"human"` | Human review + human decision |
| Reliable tests but failure needs analysis | `"cd ${worktree} && cargo test"` | `"human"` | Auto-detect, human decision |
| Simple step without tests | omit | omit | Failure is terminal, manual reset |

## Config (.pawl/config.jsonc)

```jsonc
{
  "session": "my-project",      // tmux session name (default: directory name)
  "base_branch": "main",        // base branch (default)
  "workflow": [                  // step sequence (required)
    { "name": "step-name", "run": "cmd", "verify": "human|script", "on_fail": "retry|human", "in_viewport": true, "max_retries": 3 }
  ],
  "on": { "event_name": "shell command" }  // Event hooks (optional)
}
```

## Task Definition (.pawl/tasks/{task}.md)

```yaml
---
name: my-task
depends: [other-task]    # dependencies (optional, must be Completed first)
skip: [cleanup]          # skip steps (optional, matches step name)
---

Markdown description of what needs to be done.
```

### Iterative Feedback Pattern

After failure, **append** fix guidance to the end of task.md (do not overwrite):

```markdown
---
## Fix Guidance (Round 2)

Previous issue: test_refresh_token assertion error
Fix: Extract token generation into a pure function, pass fixed time in tests
```

Append instead of overwrite: preserves history to avoid repeating mistakes.

## Variables

All `run`/`verify`/hook commands support `${var}` expansion, subprocesses get `PAWL_VAR` environment variables:

| Variable | Value |
|----------|-------|
| `${task}` / `${branch}` | Task name / `pawl/{task}` |
| `${worktree}` | `{repo_root}/{worktree_dir}/{task}` |
| `${session}` | Viewport session name |
| `${repo_root}` | Repository root directory |
| `${step}` / `${step_index}` | Current step name / index (0-based) |
| `${base_branch}` | Base branch |
| `${log_file}` / `${task_file}` | `.pawl/logs/{task}.jsonl` / `.pawl/tasks/{task}.md` |

## State Machine

```
Pending → Running → Waiting    (awaits pawl done)
                  → Completed  (all steps done)
                  → Failed     (step failed / viewport lost)
                  → Stopped    (pawl stop)
```

**Step indexing**: CLI human-readable output is 1-based (`[1/5] build`), all programmatic interfaces are 0-based (`--step 0`, `--json`, JSONL, `PAWL_STEP_INDEX`).

## Event Hooks

Configured in config's `"on"` field. **Fire-and-forget async execution** (does not wait for result, failures are silent, does not affect main flow).

### Event-Variable Mapping

| Event | Extra Variables | Trigger |
|-------|----------------|---------|
| `task_started` | — | Task started |
| `step_finished` | `${success}`, `${exit_code}`, `${duration}` | Step finished (success or failure) |
| `step_yielded` | `${reason}` (gate/verify_human/on_fail_human) | Step yielded for human input |
| `step_resumed` | — | `pawl done` resumed |
| `viewport_launched` | — | in_viewport command sent to viewport |
| `step_skipped` | — | Step skipped |
| `step_reset` | `${auto}` (true=auto retry/false=manual) | Step reset |
| `viewport_lost` | — | viewport disappeared |
| `task_stopped` | — | `pawl stop` |
| `task_reset` | — | `pawl reset` |

All hooks also have access to standard variables (`${task}`, `${step}`, `${session}`, etc.).

### Hook Examples

```jsonc
// Write to log file
"on": { "step_finished": "echo '[${task}] ${step} exit=${exit_code}' >> ${repo_root}/.pawl/hook.log" }

// Notify a supervisor via tmux (concurrency-safe)
"on": { "step_finished": "mkdir /tmp/pawl-notify.lock 2>/dev/null && tmux send-keys -t ${session}:supervisor -l '[pawl] ${task}/${step} finished (exit=${exit_code})' && tmux send-keys -t ${session}:supervisor C-Enter && sleep 0.3 && rmdir /tmp/pawl-notify.lock; true" }
```

## Config Recipes

### Recipe 1: Git Worktree + Custom Worker

```jsonc
{
  "workflow": [
    { "name": "setup",   "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    { "name": "work",    "run": "cd ${worktree} && ./run-worker.sh",
      "in_viewport": true, "verify": "cd ${worktree} && npm test", "on_fail": "retry", "max_retries": 3 },
    { "name": "review" },
    { "name": "merge",   "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge from pawl'" },
    { "name": "cleanup", "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true" }
  ],
  "on": { "step_finished": "echo '[pawl] ${task}/${step} exit=${exit_code}' >> ${repo_root}/.pawl/hook.log" }
}
```

### Recipe 2: Human Review Flow

```jsonc
{
  "workflow": [
    { "name": "setup",   "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    { "name": "work",    "run": "cd ${worktree} && ./run-worker.sh",
      "in_viewport": true, "verify": "human", "on_fail": "human" },
    { "name": "merge",   "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge'" },
    { "name": "cleanup", "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true" }
  ]
}
```

### Recipe 3: Pure Automation (No Viewport)

```jsonc
{
  "workflow": [
    { "name": "setup",   "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    { "name": "build",   "run": "cd ${worktree} && make build", "on_fail": "retry", "max_retries": 2 },
    { "name": "test",    "run": "cd ${worktree} && make test",  "on_fail": "human" },
    { "name": "review",  "verify": "human" },
    { "name": "merge",   "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge'" },
    { "name": "cleanup", "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true" }
  ]
}
```

### Recipe 4: Generic Pipeline (No Git)

pawl is a generic step sequencer — git worktrees are one pattern, not a requirement:

```jsonc
{
  "workflow": [
    { "name": "prepare", "run": "mkdir -p ~/workspace/${task} && cd ~/workspace/${task} && ./init.sh" },
    { "name": "execute", "run": "cd ~/workspace/${task} && ./run.sh",
      "in_viewport": true, "verify": "human", "on_fail": "human" },
    { "name": "validate" },
    { "name": "teardown", "run": "rm -rf ~/workspace/${task}" }
  ]
}
```

### Recipe 5: Multi-Task Parallel + Notification

```jsonc
{
  "session": "my-project",
  "workflow": [
    { "name": "setup",   "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    { "name": "work",    "run": "cd ${worktree} && ./run-worker.sh",
      "in_viewport": true, "verify": "cd ${worktree} && make test", "on_fail": "retry", "max_retries": 3 },
    { "name": "review" },
    { "name": "merge",   "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge'" },
    { "name": "cleanup", "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true" }
  ],
  "on": {
    "step_finished": "mkdir /tmp/pawl-notify.lock 2>/dev/null && tmux send-keys -t ${session}:supervisor -l '[pawl] ${task}/${step} finished (exit=${exit_code})' && tmux send-keys -t ${session}:supervisor C-Enter && sleep 0.3 && rmdir /tmp/pawl-notify.lock; true",
    "step_yielded": "mkdir /tmp/pawl-notify.lock 2>/dev/null && tmux send-keys -t ${session}:supervisor -l '[pawl] ${task} yielded: ${reason}' && tmux send-keys -t ${session}:supervisor C-Enter && sleep 0.3 && rmdir /tmp/pawl-notify.lock; true"
  }
}
```

Start multiple tasks: `pawl start task-a && pawl start task-b && pawl start task-c`. Each gets independent JSONL/worktree/viewport.

## AI Worker Integration Recipes

pawl is tool-agnostic. Below are patterns for integrating AI coding agents as viewport workers. The worker is responsible for its own session management — pawl only provides the sequencing, variables, and event log.

### Recipe: Claude Code Worker

```jsonc
{ "name": "develop",
  "run": "cd ${worktree} && SF=.claude-session; if [ -f $SF ]; then FB=$(grep '\"step_finished\"' $PAWL_LOG_FILE | grep '\"success\":false' | tail -1 | jq -r '.verify_output // empty' 2>/dev/null); claude -p \"Fix: $FB\" -r $(cat $SF) --output-format json | jq -r '.session_id' > $SF; else cat $PAWL_TASK_FILE | claude -p - --output-format json | jq -r '.session_id' > $SF; fi",
  "in_viewport": true, "verify": "cd ${worktree} && npm test", "on_fail": "retry" }
```

Key points:
- Session ID stored in `${worktree}/.claude-session`, managed by the worker itself
- On retry, reads `PAWL_LOG_FILE` to extract failure feedback
- `-r $(cat $SF)` resumes context, avoids re-understanding codebase

### Recipe: Codex CLI Worker

```jsonc
{ "name": "develop",
  "run": "cd ${worktree} && FB=$(grep '\"step_finished\"' $PAWL_LOG_FILE | grep '\"success\":false' | tail -1 | jq -r '.verify_output // empty' 2>/dev/null); if [ -n \"$FB\" ]; then codex -q --full-auto \"Fix: $FB\"; else codex -q --full-auto \"$(cat $PAWL_TASK_FILE)\"; fi",
  "in_viewport": true, "verify": "cd ${worktree} && npm test", "on_fail": "retry" }
```

### Recipe: Generic Agent Worker

Any CLI that accepts a prompt and runs non-interactively works:

```jsonc
{ "name": "develop",
  "run": "cd ${worktree} && cat $PAWL_TASK_FILE | your-agent-cli --non-interactive",
  "in_viewport": true, "verify": "human", "on_fail": "human" }
```

### Recipe: Plan-Then-Execute Pattern

Split planning and execution into separate steps. The plan step runs the AI in read-only mode; a human reviews; then the execute step implements.

```jsonc
{
  "workflow": [
    { "name": "setup", "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    { "name": "plan",  "run": "cd ${worktree} && cat ${task_file} | claude -p - --permission-mode plan --output-format json | jq -r '.session_id' > .claude-session",
      "in_viewport": true, "verify": "human", "on_fail": "human" },
    { "name": "develop", "run": "cd ${worktree} && claude -p 'Execute the approved plan.' -r $(cat .claude-session)",
      "in_viewport": true, "verify": "cd ${worktree} && cargo test", "on_fail": "retry", "max_retries": 3 },
    { "name": "review" },
    { "name": "merge",   "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge'" },
    { "name": "cleanup", "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true" }
  ]
}
```

Plan rejection: `pawl reset --step` on the plan step re-plans from scratch.

## Supervisor Coordination

A supervisor (human or AI agent) manages multiple workers. pawl **does not push notifications — the supervisor must poll** (unless event hooks are configured).

### Main Loop

```
while tasks remain incomplete:
    1. pawl list                          # scan global status
    2. for each waiting task:
       - gate → pawl done (confirm preconditions met)
       - verify_human → pawl capture/pawl log to review output → pawl done or pawl reset --step
       - on_fail_human → pawl status --json for last_feedback → fix then pawl reset --step
    3. for each failed task:
       - pawl status --json for last_feedback + retry_count
       - fixable → pawl reset --step    unfixable → pawl start --reset or pawl stop
    4. for each running + in_viewport task:
       - pawl capture to check progress   pawl enter if direct interaction needed
    5. sleep / wait for event hook notification
```

### Status Decision Table

| status | message | Action |
|--------|---------|--------|
| pending | — | `pawl start <task>` (check blocked_by first) |
| running | — | No action needed (`pawl capture` to monitor in_viewport) |
| waiting | gate | `pawl done <task>` (confirm gate conditions) |
| waiting | verify_human | Review output → `pawl done` or `pawl reset --step` |
| waiting | on_fail_human | Analyze feedback → `pawl done`(approve) / `reset --step`(retry) / `stop`(abandon) |
| failed | exit code/msg | `pawl status --json` for feedback → fix → `pawl reset --step` |
| stopped | — | `pawl start --reset` (start over) |
| completed | — | No action needed |

### Key Constraints

- **viewport_lost is passive detection**: pawl does not proactively notify when a viewport disappears. Detection only happens when `pawl status`/`pawl list`/`pawl wait` is called. Periodically `pawl list` to check health of in_viewport steps.
- **pawl done dual semantics**: For Waiting status = approve (step advances); for Running+in_viewport = mark done (triggers verify flow).
- **Retry exhaustion**: After reaching max_retries, status becomes Failed (does not auto-transition to Waiting). Manual intervention required.

## pawl status --json Output

### List Mode (no task argument)

```json
[{
  "name": "my-task",
  "status": "waiting",
  "current_step": 2,
  "total_steps": 6,
  "step_name": "review",
  "message": "verify_human",
  "started_at": "RFC3339",
  "updated_at": "RFC3339",
  "blocked_by": ["dep-task"],
  "retry_count": 0,
  "last_feedback": "string"
}]
```

### Single Task Detail (with task argument)

Adds `description`, `depends`, `workflow` fields:

```json
{
  "workflow": [
    { "index": 0, "name": "setup", "status": "success" },
    { "index": 1, "name": "develop", "step_type": "in_viewport", "status": "current" },
    { "index": 2, "name": "review", "step_type": "gate", "status": "pending" }
  ]
}
```

Field notes: `retry_count` only counts auto retries (auto=true); `last_feedback` searches backwards, stops at TaskReset; optional fields are omitted when null. `step_type`: `"gate"` / `"in_viewport"` / omitted. `status`: `success` / `failed` / `skipped` / `current` / `pending`.

## Troubleshooting

| Symptom | Cause | Solution |
|---------|-------|----------|
| tmux session not found | Session doesn't exist | `tmux new-session -d -s <session>` |
| "Task already running" | Another pawl start is running | `pawl stop <task> && pawl start <task>` |
| Worktree already exists | Leftover from previous run | `git worktree remove .pawl/worktrees/<task> --force && git branch -D pawl/<task>` then `pawl reset` |
| viewport_lost but process alive | viewport name conflict | `tmux list-windows -t <session>` to inspect |
| Dependency blocked | Prerequisite task not completed | `pawl list` to check blocking source |
| JSONL corrupted | Write interrupted | `tail -1 .pawl/logs/<task>.jsonl` to check; `pawl reset` to reset |
