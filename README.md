# Pawl

Shell can pipe, chain, trap, and cron — but it can't **pause, wait for a decision, resume after a crash, or route failures**. pawl adds these missing primitives: a single binary that turns any shell pipeline into a resumable coroutine with failure routing.

```
  pawl start task-a     build ── test ─╳ (fail)
                                       └─ retry ── test ─── deploy ── gate ── verify
                                                                        ↑
  Close laptop. Fly across the world.                            pawl done task-a
  Reboot. pawl start task-a. Continues from gate.
```

One invariant: `state = replay(log)`. Append-only JSONL, no database, no status file.

## Install

```bash
cargo install pawl
```

Requires: Rust. Optional: tmux (only for interactive `in_viewport` steps).

## Quick Start

```bash
pawl init                    # scaffold .pawl/ with config + full reference
# edit .pawl/config.json     # define your workflow
pawl start my-task           # run the pipeline
```

## How It Works

Define a workflow in `.pawl/config.json`:

```json
{
  "workflow": [
    { "name": "build",   "run": "npm run build" },
    { "name": "test",    "run": "npm test", "on_fail": "retry" },
    { "name": "deploy",  "run": "npm run deploy" },
    { "name": "verify",  "verify": "manual" }
  ]
}
```

Four primitives compose into any workflow:

| Primitive | Config | What happens |
|-----------|--------|-------------|
| **Run** | `"run": "..."` | Execute. Non-zero exit = failure. |
| **Gate** | no `run` | Pause until `pawl done`. |
| **Retry** | `"on_fail": "retry"` | Auto-retry on failure (default: 3x). |
| **Yield** | `"verify": "manual"` or `"on_fail": "manual"` | Pause for judgment. |

Add `"in_viewport": true` to run in an interactive terminal (tmux).

### Multi-Task with Dependencies

Tasks can declare dependencies to form a DAG. pawl enforces ordering — a task won't start until its dependencies complete:

```json
{
  "tasks": {
    "lib":  { "workflow": [{ "name": "build", "run": "make lib" }] },
    "api":  { "workflow": [{ "name": "build", "run": "make api" }], "depends": ["lib"] },
    "web":  { "workflow": [{ "name": "build", "run": "make web" }], "depends": ["lib"] },
    "ship": { "workflow": [{ "name": "deploy", "run": "make deploy" }], "depends": ["api", "web"] }
  }
}
```

```bash
pawl start lib & pawl start api & pawl start web & pawl start ship
# lib runs immediately; api + web wait for lib; ship waits for both
```

## Self-Routing

stdout = JSON, stderr = progress. `pawl status` returns machine-readable routing hints — consumers don't need to understand pawl:

```bash
pawl status task-a | jq '{suggest, prompt}'
# suggest: ["pawl reset --step task-a"]     ← execute directly
# prompt:  ["verify test results, then: pawl done task-a"]  ← requires judgment
```

## Design

Three ideas, everything else follows:

1. **`state = replay(log)`** — Append-only JSONL is the single source of truth. Crash, reboot, replay, resume.
2. **Separate what from where** — Recording (what happened) and routing (what to do next) never mix.
3. **Trust the substrate** — File system, exit codes, tmux. Build only what Unix can't.

## Tests

192 tests: 136 sync E2E, 27 viewport E2E, 9 real-agent E2E, ~20 unit tests.

```bash
cargo test                    # unit tests
bash tests/e2e.sh             # sync workflows
bash tests/e2e-viewport.sh    # viewport lifecycle (requires tmux)
bash tests/e2e-agent.sh       # real AI agents (requires API key, ~$0.05)
```

## Documentation

`pawl init` generates `.pawl/README.md` — the full reference for config schema, commands, variables, and event hooks.

## Ecosystem

- [pawl-foreman](https://github.com/yansircc/agent-skills) — Claude Code skill for orchestrating AI agents with pawl

  ```bash
  /plugin marketplace add yansircc/agent-skills
  /plugin install pawl-foreman@yansircc-skills
  ```

## License

MIT
