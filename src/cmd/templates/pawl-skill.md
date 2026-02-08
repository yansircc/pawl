---
name: pawl
description: >
  Resumable step sequencer for multi-step workflows. Use when writing task definitions (author),
  designing workflow config (orchestrate), or monitoring/managing running tasks (supervise).
  Covers: task authoring, config.jsonc design, verify/retry strategies, event hooks, Claude Code
  CLI integration, supervisor coordination, and troubleshooting.
---

# pawl — Resumable Step Sequencer

A resumable coroutine: advance through a fixed step sequence, yield when unable to self-decide,
rebuild state from an append-only log. `state = replay(log)`.

- **Step**: 4 orthogonal properties (`run`, `verify`, `on_fail`, `in_viewport`) — see config comments
- **Gate step**: No `run` — pauses for `pawl done`
- **Yield**: Step can't self-decide → waits for human/supervisor input
- **Replay**: All state derived from JSONL log, no separate storage

Run `pawl --help` for CLI reference, variables, states, and indexing.
Step properties, design rules, and event hooks are in `.pawl/config.jsonc` comments.

## Roles

| Role | When | Reference |
|------|------|-----------|
| **Author** | Writing task definitions (`.pawl/tasks/*.md`) | [author.md](references/author.md) |
| **Orchestrator** | Designing workflow config (`.pawl/config.jsonc`) | [orchestrate.md](references/orchestrate.md) |
| **Supervisor** | Monitoring and managing running tasks | [supervise.md](references/supervise.md) |
