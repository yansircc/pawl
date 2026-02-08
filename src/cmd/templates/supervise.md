# Supervisor — Monitoring and Managing Tasks

A supervisor (human or AI agent) manages multiple workers. pawl **does not push notifications — the supervisor must poll** (unless event hooks are configured).

## Main Loop

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

## Status Decision Table

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

## Key Constraints

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
