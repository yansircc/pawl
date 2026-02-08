# Supervisor — Monitoring and Troubleshooting

## States

Pending → Running → Waiting / Completed / Failed / Stopped

## Status Fields

`pawl status [task]` JSON output:
- `name`, `status`, `current_step` (0-based), `total_steps`, `step_name`
- `message`, `blocked_by`, `retry_count`, `last_feedback`
- `suggest`, `prompt` (routing — see below)
- With task arg: adds `description`, `depends`, `workflow` (array of `{index, name, status, step_type}`)
- Optional fields omitted when empty/null

## Routing Hints

`pawl status`/`pawl list` JSON output includes two routing fields:

- **suggest**: mechanical commands — execute directly (e.g. `pawl reset --step foo`)
- **prompt**: requires judgment — evaluate context, then decide (e.g. "verify work quality, then: pawl done foo")
- `pawl done` never appears in suggest — it always requires judgment

## Monitoring

### Poll (multi-task, default)

```
while tasks remain incomplete:
    1. pawl list                              # JSON array with suggest/prompt
    2. for each task: follow suggest / evaluate prompt
    3. sleep / wait for event hook notification
```

### Event-driven (real-time)

`pawl events --follow [task]` — tails JSONL events as they arrive. More efficient for long-running tasks than polling.

### Blocking (single-task)

`pawl wait <task> --until waiting,failed` — blocks until target status reached. Simplest for single-task scripts.

## Key Constraints

- **viewport_lost is passive**: only detected when `pawl status`/`pawl list`/`pawl wait` is called. Poll periodically to catch in_viewport failures.
- **pawl done dual semantics**: Waiting = approve (step advances); Running+in_viewport = mark done (triggers verify flow).
- **Retry exhaustion**: after max_retries, status becomes Failed (not Waiting). Manual intervention required.
- **Viewport debugging**: `pawl capture <task>` captures current viewport content as JSON.

## Troubleshooting

| Symptom | Cause | Solution |
|---------|-------|----------|
| tmux session not found | Session doesn't exist | `tmux new-session -d -s <session>` |
| "Task already running" | Another pawl start is running | `pawl stop <task> && pawl start <task>` |
| viewport_lost but process alive | viewport name conflict | `tmux list-windows -t <session>` to inspect |
| Dependency blocked | Prerequisite task not completed | `pawl list` to check blocking source |
| JSONL corrupted | Write interrupted | `tail -1 .pawl/logs/<task>.jsonl` to check; `pawl reset` to reset |
