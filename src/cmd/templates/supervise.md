# Supervisor — Polling and Troubleshooting

pawl **does not push notifications — the supervisor must poll** (unless event hooks are configured).

## Main Loop

```
while tasks remain incomplete:
    1. pawl list                              # scan all tasks (JSON array)
    2. for each task: follow suggest / evaluate prompt
    3. sleep / wait for event hook notification
```

- **suggest**: mechanical next commands — execute directly
- **prompt**: requires judgment — evaluate context before deciding
- `pawl done` never appears in suggest (it requires judgment, not routing)

## Key Constraints

- **viewport_lost is passive detection**: pawl only detects viewport disappearance when `pawl status`/`pawl list`/`pawl wait` is called. Poll periodically to catch in_viewport failures.
- **pawl done dual semantics**: For Waiting = approve (step advances); for Running+in_viewport = mark done (triggers verify flow).
- **Retry exhaustion**: After reaching max_retries, status becomes Failed (not Waiting). Manual intervention required.

## Troubleshooting

| Symptom | Cause | Solution |
|---------|-------|----------|
| tmux session not found | Session doesn't exist | `tmux new-session -d -s <session>` |
| "Task already running" | Another pawl start is running | `pawl stop <task> && pawl start <task>` |
| Worktree already exists | Leftover from previous run | `git worktree remove .pawl/worktrees/<task> --force && git branch -D pawl/<task>` then `pawl reset` |
| viewport_lost but process alive | viewport name conflict | `tmux list-windows -t <session>` to inspect |
| Dependency blocked | Prerequisite task not completed | `pawl list` to check blocking source |
| JSONL corrupted | Write interrupted | `tail -1 .pawl/logs/<task>.jsonl` to check; `pawl reset` to reset |
