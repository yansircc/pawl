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

### Wait (preferred)

`pawl wait <task> --until <status>[,status2] [-t sec]` blocks until target status. Run multiple in parallel for multi-task:

```bash
# Wait for all tasks to reach review gate (or fail)
pawl wait task-a --until waiting,completed,failed &
pawl wait task-b --until waiting,completed,failed &
pawl wait task-c --until waiting,completed,failed &
wait
# All settled — review and approve
pawl list
```

### Events (real-time)

`pawl events --follow [task]` tails JSONL events as they arrive. Use as a live dashboard — see every step_finished, step_yielded, step_reset the moment it happens. Filter with `--type`:

```bash
pawl events --follow --type step_yielded,step_finished
```

### Poll (fallback)

`pawl list` for a one-shot status check. Only use repeated polling when wait/events are impractical:

```
pawl list                              # JSON array with suggest/prompt
for each task: follow suggest / evaluate prompt
```

## Key Constraints

- **viewport failure has two paths**: (1) Normal: viewport killed → `_run` catches child exit → `step_finished(exit_code=128)` → Failed. (2) Safety net: `_run` itself crashed/SIGKILL'd → `viewport_lost` emitted passively by `pawl status`/`list`/`wait`. Most viewport failures are path 1. Poll periodically to catch path 2.
- **pawl done dual semantics**: Waiting = approve (step advances); Running+in_viewport = mark done (triggers verify flow).
- **Retry exhaustion**: after max_retries, status becomes Failed (not Waiting). Manual intervention required.

## Troubleshooting

| Symptom | Cause | Solution |
|---------|-------|----------|
| tmux session not found | Session doesn't exist | `tmux new-session -d -s <session>` |
| "Task already running" | Another pawl start is running | `pawl stop <task> && pawl start <task>` |
| viewport_lost but process alive | viewport name conflict | `tmux list-windows -t <session>` to inspect |
| Dependency blocked | Prerequisite task not completed | `pawl list` to check blocking source |
| JSONL corrupted | Write interrupted | `tail -1 .pawl/logs/<task>.jsonl` to check; `pawl reset` to reset |
