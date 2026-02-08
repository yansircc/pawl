# pawl — Resumable Step Sequencer

Run `pawl --help` for CLI reference, variables, and state machine. Key subcommands have detailed `--help` (e.g. `pawl status --help` for JSON fields, `pawl done --help` for dual semantics).

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

### Iterative Feedback Pattern

After failure, **append** fix guidance to the end of task.md (do not overwrite):

```markdown
---
## Fix Guidance (Round 2)

Previous issue: test_refresh_token assertion error
Fix: Extract token generation into a pure function, pass fixed time in tests
```

Append instead of overwrite: preserves history to avoid repeating mistakes.

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

### Git Worktree Skeleton

Recipes 1–3 and Plan-Then-Execute share this skeleton. Replace `⟨work⟩` with a variant below; omit `review` gate if unneeded; add `"on": {...}` for hooks (see Hook Examples above).

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

Multi-task: `pawl start task-a && pawl start task-b && pawl start task-c` — each gets independent JSONL/worktree/viewport.

### Variant: Viewport + Auto Verify

```jsonc
{ "name": "work", "run": "cd ${worktree} && ./run-worker.sh",
  "in_viewport": true, "verify": "cd ${worktree} && npm test", "on_fail": "retry", "max_retries": 3 }
```

### Variant: Viewport + Human Review

```jsonc
{ "name": "work", "run": "cd ${worktree} && ./run-worker.sh",
  "in_viewport": true, "verify": "human", "on_fail": "human" }
```

Omit the `review` gate (human verify on work step already covers it).

### Variant: Sync Steps (No Viewport)

```jsonc
{ "name": "build", "run": "cd ${worktree} && make build", "on_fail": "retry", "max_retries": 2 },
{ "name": "test",  "run": "cd ${worktree} && make test",  "on_fail": "human" }
```

Add `"verify": "human"` to the `review` step.

### Recipe: Generic Pipeline (No Git)

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

## AI Worker Integration Recipes

pawl is tool-agnostic. The worker manages its own session — pawl provides sequencing, variables, and event log. Use these as `⟨work⟩` in the git worktree skeleton above.

### Recipe: Claude Code Worker

```jsonc
{ "name": "develop",
  "run": "cd ${worktree} && SF=.claude-session; if [ -f $SF ]; then FB=$(grep '\"step_finished\"' $PAWL_LOG_FILE | grep '\"success\":false' | tail -1 | jq -r '.verify_output // empty' 2>/dev/null); claude -p \"Fix: $FB\" -r $(cat $SF) --output-format json | jq -r '.session_id' > $SF; else cat $PAWL_TASK_FILE | claude -p - --output-format json | jq -r '.session_id' > $SF; fi",
  "in_viewport": true, "verify": "cd ${worktree} && npm test", "on_fail": "retry" }
```

- Session ID in `${worktree}/.claude-session`, managed by worker
- On retry, reads `PAWL_LOG_FILE` for failure feedback; `-r $(cat $SF)` resumes context

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

### Recipe: Plan-Then-Execute

Use with the git worktree skeleton, replacing `⟨work⟩` with both steps:

```jsonc
{ "name": "plan", "run": "cd ${worktree} && cat ${task_file} | claude -p - --permission-mode plan --output-format json | jq -r '.session_id' > .claude-session",
  "in_viewport": true, "verify": "human", "on_fail": "human" },
{ "name": "develop", "run": "cd ${worktree} && claude -p 'Execute the approved plan.' -r $(cat .claude-session)",
  "in_viewport": true, "verify": "cd ${worktree} && cargo test", "on_fail": "retry", "max_retries": 3 }
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

## Troubleshooting

| Symptom | Cause | Solution |
|---------|-------|----------|
| tmux session not found | Session doesn't exist | `tmux new-session -d -s <session>` |
| "Task already running" | Another pawl start is running | `pawl stop <task> && pawl start <task>` |
| Worktree already exists | Leftover from previous run | `git worktree remove .pawl/worktrees/<task> --force && git branch -D pawl/<task>` then `pawl reset` |
| viewport_lost but process alive | viewport name conflict | `tmux list-windows -t <session>` to inspect |
| Dependency blocked | Prerequisite task not completed | `pawl list` to check blocking source |
| JSONL corrupted | Write interrupted | `tail -1 .pawl/logs/<task>.jsonl` to check; `pawl reset` to reset |
