# pawl — Resumable Step Sequencer

pawl is a **resumable coroutine**: advance along a fixed step sequence, yield at decision points, rebuild state from an append-only log. Any repeatable multi-step process can be a pawl workflow — AI coding with git worktrees, testing pipelines, deployment automation, project bootstrapping. Steps support verify/retry/gate for human-in-the-loop control; viewports for long-running processes.

## CLI Commands

| Command | Description |
|---------|-------------|
| `pawl init` | Initialize project (creates .pawl/ and .claude/skills/pawl/) |
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
  "claude_command": "claude",   // Claude CLI command (default: "claude", change to "ccc" etc. for aliases)
  "workflow": [                  // step sequence (required)
    { "name": "step-name", "run": "cmd", "verify": "human|script", "on_fail": "retry|human", "in_viewport": true, "max_retries": 3 }
  ],
  "on": { "event_name": "shell command" }  // Event hooks (optional)
}
```

## Task Definition (.pawl/tasks/{task}.md)

Task.md has a **dual role**: human documentation + AI Worker system prompt (injected via `cat task.md | claude -p`).

```yaml
---
name: my-task
depends: [other-task]    # dependencies (optional, must be Completed first)
skip: [cleanup]          # skip steps (optional, matches step name)
---

Markdown description (also serves as AI Worker prompt)
```

### Iterative Feedback Pattern

After failure, **append** fix guidance to the end of task.md (do not overwrite):

```markdown
---
## Fix Guidance (Round 2)

Previous issue: test_refresh_token assertion error
Fix: Extract token generation into a pure function, pass fixed time in tests
```

Append instead of overwrite: preserves history to avoid repeating mistakes, Worker can see prior error context.

## Variables

All `run`/`verify`/hook commands support `${var}` expansion, subprocesses get `PAWL_VAR` environment variables:

| Variable | Value |
|----------|-------|
| `${task}` / `${branch}` | Task name / `pawl/{task}` |
| `${worktree}` | `{repo_root}/{worktree_dir}/{task}` |
| `${session}` | tmux session name |
| `${repo_root}` | Repository root directory |
| `${step}` / `${step_index}` | Current step name / index (0-based) |
| `${base_branch}` | Base branch |
| `${claude_command}` | Claude CLI command (from config, default "claude") |
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
| `step_completed` | `${exit_code}`, `${duration}` | Step completed (success or failure) |
| `step_waiting` | `${reason}` (gate/verify_human/on_fail_human) | Step paused for human input |
| `step_approved` | — | `pawl done` approved |
| `viewport_launched` | — | in_viewport command sent to viewport |
| `step_skipped` | — | Step skipped |
| `step_reset` | `${auto}` (true=auto retry/false=manual) | Step reset |
| `viewport_lost` | — | viewport disappeared |
| `task_stopped` | — | `pawl stop` |
| `task_reset` | — | `pawl reset` |

All hooks also have access to standard variables (`${task}`, `${step}`, `${session}`, etc.).

### Hook Examples

```jsonc
// Write to log file (simplest)
"on": { "step_completed": "echo '[${task}] ${step} exit=${exit_code}' >> ${repo_root}/.pawl/hook.log" }

// Notify Foreman TUI (concurrency-safe)
"on": { "step_completed": "mkdir /tmp/pawl-notify.lock 2>/dev/null && tmux send-keys -t ${session}:foreman -l '[pawl] ${task}/${step} done (exit=${exit_code})' && tmux send-keys -t ${session}:foreman C-Enter && sleep 0.3 && rmdir /tmp/pawl-notify.lock; true" }
```

Foreman notification details: `mkdir` atomic mutex prevents concurrent interleaving; `-l` sends literal text; `C-Enter` submits to Claude Code TUI input; `sleep 0.3` ensures atomicity.

## Config Recipes

### Recipe 1: Basic AI Development Flow

```jsonc
{
  "workflow": [
    { "name": "setup",   "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    { "name": "develop", "run": "source ${repo_root}/.pawl/lib/ai-helpers.sh && cd ${worktree} && run_ai_worker",
      "in_viewport": true, "verify": "cd ${worktree} && npm test", "on_fail": "retry", "max_retries": 3 },
    { "name": "review" },
    { "name": "merge",   "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge from pawl'" },
    { "name": "cleanup", "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true" }
  ],
  "on": { "step_completed": "echo '[pawl] ${task}/${step} exit=${exit_code}' >> ${repo_root}/.pawl/hook.log" }
}
```

Notes: develop has verify+on_fail+cd worktree (satisfies all 3 rules); review is a pure gate (no run).

### Recipe 2: Full Human Review Flow

```jsonc
{
  "workflow": [
    { "name": "setup",   "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    { "name": "develop", "run": "source ${repo_root}/.pawl/lib/ai-helpers.sh && cd ${worktree} && run_ai_worker",
      "in_viewport": true, "verify": "human", "on_fail": "human" },
    { "name": "merge",   "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge'" },
    { "name": "cleanup", "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true" }
  ]
}
```

Notes: verify=human lets Foreman review output; on_fail=human lets Foreman decide retry/abandon.

### Recipe 3: Auto Verify + Escalate to Human on Retry Exhaustion

```jsonc
{
  "workflow": [
    { "name": "setup",   "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    { "name": "develop", "run": "source ${repo_root}/.pawl/lib/ai-helpers.sh && cd ${worktree} && run_ai_worker",
      "in_viewport": true, "verify": "cd ${worktree} && cargo test", "on_fail": "retry", "max_retries": 3 },
    { "name": "final-review" },
    { "name": "merge",   "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge'" },
    { "name": "cleanup", "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true" }
  ]
}
```

Retry exhaustion behavior: after 3 failed retries, status becomes Failed. Foreman checks `last_feedback` via `pawl status --json`, fixes the issue, then `pawl reset --step` to continue. final-review is a gate ensuring human confirmation before merge.

### Recipe 4: Multi-Agent Parallel + Foreman Notification

```jsonc
{
  "session": "my-project",
  "workflow": [
    { "name": "setup",   "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    { "name": "develop", "run": "source ${repo_root}/.pawl/lib/ai-helpers.sh && cd ${worktree} && run_ai_worker",
      "in_viewport": true, "verify": "cd ${worktree} && make test", "on_fail": "retry", "max_retries": 3 },
    { "name": "review" },
    { "name": "merge",   "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge'" },
    { "name": "cleanup", "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true" }
  ],
  "on": {
    "step_completed": "mkdir /tmp/pawl-notify.lock 2>/dev/null && tmux send-keys -t ${session}:foreman -l '[pawl] ${task}/${step} done (exit=${exit_code})' && tmux send-keys -t ${session}:foreman C-Enter && sleep 0.3 && rmdir /tmp/pawl-notify.lock; true",
    "step_waiting": "mkdir /tmp/pawl-notify.lock 2>/dev/null && tmux send-keys -t ${session}:foreman -l '[pawl] ${task} waiting: ${reason}' && tmux send-keys -t ${session}:foreman C-Enter && sleep 0.3 && rmdir /tmp/pawl-notify.lock; true"
  }
}
```

Start multiple tasks in parallel: `pawl start task-a && pawl start task-b && pawl start task-c`. Each task has independent JSONL/worktree/viewport and does not interfere with others. Event hooks notify the Foreman viewport via tmux send-keys.

### Recipe 5: Pure Automation Flow (No AI)

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

Notes: no in_viewport or ai-helpers.sh, pure synchronous commands. review is a gate + verify=human combo: first gate waits for pawl done, then verify_human waits for pawl done again.

### Recipe 6: Generic Pipeline (No Git Worktrees)

pawl is a generic step sequencer — git worktrees are one pattern, not a requirement. Use `${task}` as any identifier (project name, test scenario, deployment target):

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

No `${worktree}`, `${branch}`, or git operations. Examples: testing pipelines (task = test case), deployment (task = service), data processing (task = dataset), project bootstrapping (task = project name).

### Recipe 7: Plan-First Development (Foreman Reviews Plan Before Execution)

Adds explicit plan approval step. AI creates a plan in read-only mode, foreman reviews and approves before any code is written. Requires one-time setup: `cd .pawl/lib && npm install`.

```jsonc
{
  "workflow": [
    { "name": "setup", "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    { "name": "plan",
      "run": "cd ${worktree} && node ${repo_root}/.pawl/lib/plan-worker.mjs",
      "in_viewport": true, "verify": "human", "on_fail": "human" },
    { "name": "develop",
      "run": "source ${repo_root}/.pawl/lib/ai-helpers.sh && cd ${worktree} && run_ai_worker",
      "in_viewport": true, "verify": "cd ${worktree} && cargo test", "on_fail": "retry", "max_retries": 3 },
    { "name": "review" },
    { "name": "merge", "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge'" },
    { "name": "cleanup", "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true" }
  ]
}
```

Notes: plan step runs AI in read-only plan mode via Claude Agent SDK. When AI calls `ExitPlanMode`, the plan is saved to `.pawl/plans/${task}.md` and the session ID to `.pawl/plans/${task}.session`. Foreman reviews the plan then `pawl done` to approve. The develop step's `run_ai_worker` detects the plan session file and resumes it with `-r session_id`, executing the approved plan. Plan rejection: `pawl reset --step` on the plan step re-plans from scratch.

## Foreman Coordination

Foreman is an AI agent that manages multiple worker agents. pawl **does not push notifications — Foreman must poll** (unless event hooks are configured).

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

## AI Worker Integration (Coding Workflow Pattern)

This section covers the AI coding workflow pattern specifically. For non-AI or non-coding use cases, see Recipe 5 (pure automation) or Recipe 6 (generic pipeline).

### run_ai_worker Decision Flow

```
extract_session_id(JSONL)
├─ no session_id → first run: cat task.md | claude -p - --tools "Bash,Read,Write"
└─ has session_id → resume:
   ├─ extract_feedback(JSONL) to get failure feedback
   └─ claude -p "Fix: $feedback" -r $session_id --tools "Bash,Read,Write"
```

Value of resumption: avoids re-understanding the codebase from scratch on each retry. `-r session_id` preserves prior session context.

### run_ai_worker Parameters

| Option | Default | Description |
|--------|---------|-------------|
| `--log-file <path>` | `$PAWL_LOG_FILE` | JSONL log path |
| `--task-file <path>` | `$PAWL_TASK_FILE` | Task markdown path |
| `--tools <tools>` | `Bash,Read,Write` | Comma-separated tool list |
| `--claude-cmd <cmd>` | `$PAWL_CLAUDE_COMMAND` or `claude` | Claude CLI command |
| `--extra-args <args>` | (empty) | Extra arguments passed to claude |

### Typical in_viewport Step Config

```jsonc
// Basic
{ "run": "source ${repo_root}/.pawl/lib/ai-helpers.sh && cd ${worktree} && run_ai_worker", "in_viewport": true }

// Custom tools and model
{ "run": "source ${repo_root}/.pawl/lib/ai-helpers.sh && cd ${worktree} && run_ai_worker --tools 'Bash,Read,Write,Edit' --extra-args '--model sonnet'", "in_viewport": true }
```

### Custom Wrapper

When `run_ai_worker` doesn't meet your needs, bypass it and call claude directly:

```bash
#!/bin/bash
source "$PAWL_REPO_ROOT/.pawl/lib/ai-helpers.sh"
cd "$PAWL_WORKTREE"
sid=$(extract_session_id "$PAWL_LOG_FILE")
if [ -n "$sid" ]; then
    feedback=$(extract_feedback "$PAWL_LOG_FILE")
    claude -p "Previous error: $feedback. Fix it." -r "$sid" --tools "Bash,Read,Write"
else
    cat "$PAWL_TASK_FILE" | claude -p - --tools "Bash,Read,Write,Edit"
fi
```

Reference in config: `"run": "bash ${repo_root}/.pawl/lib/my-wrapper.sh"`

### Claude CLI Key Flags

| Flag | Purpose |
|------|---------|
| `-p` | Non-interactive mode (required) |
| `-r <session_id>` | Resume session (preserves context) |
| `--tools "T1,T2"` | Available tool set |
| `--output-format json` | JSON output (includes session_id) |
| `--model <name>` | Specify model |

Constraint: `-r session_id` must be in the same cwd (session data is stored per project directory). pawl's worktree path is deterministic, satisfying this constraint.

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
| `-r session_id` fails | cwd mismatch | Must run in same worktree directory |
| JSONL corrupted | Write interrupted | `tail -1 .pawl/logs/<task>.jsonl` to check; `pawl reset` to reset |
