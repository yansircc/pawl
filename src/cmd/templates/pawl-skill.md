---
name: pawl
description: >
  Resumable step sequencer for multi-step workflows. Use when writing task definitions (author),
  designing workflow config (orchestrate), or monitoring/managing running tasks (supervise).
  Covers: task authoring, config.jsonc design, verify/retry strategies, event hooks, Claude Code
  CLI integration, supervisor coordination, and troubleshooting.
---

# pawl — Agent-Friendly Resumable Step Sequencer

A resumable coroutine whose consumer is agents, not humans. Advance through a fixed step sequence,
yield when unable to self-decide, rebuild state from an append-only log. `state = replay(log)`.
stdout = JSON/JSONL. stderr = plain text (errors + progress). `pawl status` includes routing hints (`suggest`/`prompt`).

- **Step**: 4 orthogonal properties (`run`, `verify`, `on_fail`, `in_viewport`) — see orchestrate.md
- **Gate step**: No `run` — pauses for `pawl done`
- **Yield**: Step can't self-decide → waits for external input
- **Replay**: All state derived from JSONL log, no separate storage

States: Pending → Running → Waiting / Completed / Failed / Stopped. Indexing: 0-based everywhere.

## Roles

| Role | When | Reference |
|------|------|-----------|
| **Orchestrator** | Designing workflow config (`.pawl/config.jsonc`) | [orchestrate.md](references/orchestrate.md) |
| **Author** | Writing task definitions (`.pawl/tasks/*.md`) | [author.md](references/author.md) |
| **Supervisor** | Polling and troubleshooting | [supervise.md](references/supervise.md) |
