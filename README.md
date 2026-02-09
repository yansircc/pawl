# pawl

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

Requires: Rust, tmux.

## 30 Seconds

```bash
pawl init                    # scaffold .pawl/
# edit .pawl/config.json    # define workflow
pawl create my-task          # create a task
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
| **Yield** | `"verify": "manual"` or `"on_fail": "manual"` | Pause for human/agent judgment. |

Add `"in_viewport": true` to run in an interactive terminal (tmux).

## Agent Orchestration

pawl's strongest emergent property: **durable execution for AI agent swarms**.

Agent teams (Claude Code, multi-agent frameworks) coordinate agents in memory — if the process dies, the state is gone. pawl adds the missing durability layer: crash-recoverable state, self-routing hints, and recursive supervision.

### Self-Routing Protocol

`pawl status` returns machine-readable routing hints. Agents don't need to understand pawl — pawl tells them what to do:

```bash
pawl status task-a | jq '{suggest, prompt}'
# suggest: ["pawl reset --step task-a"]     ← execute directly
# prompt:  ["verify test results, then: pawl done task-a"]  ← requires judgment
```

### Single Agent with Retry Loop

```json
{
  "workflow": [
    { "name": "implement", "run": "cat ${task_file} | claude -p --session-id $PAWL_RUN_ID",
      "in_viewport": true, "verify": "npm test", "on_fail": "retry" }
  ]
}
```

Agent implements → tests verify → fail? pawl auto-retries with `$PAWL_LAST_VERIFY_OUTPUT` as feedback. `$PAWL_RUN_ID` is stable across retries — session context survives.

### Recursive Supervision

Each level of an agent hierarchy can have its own pawl — supervisor trees for shell:

```
Leader (pawl: spawn → gate → synthesize)
  ├─ Worker A (pawl: plan → implement → test)
  ├─ Worker B (pawl: plan → implement → test)
  └─ Worker C (pawl: plan → implement → test)
```

Leader's pawl spawns workers, each worker has its own pawl workflow. Worker B fails step 2? Its pawl retries 3x, then yields. Leader's event hook notices, routes the decision. Everything is in JSONL — crash the whole machine, reboot, every workflow resumes from where it was.

### Event Hooks for Coordination

```json
"on": {
  "step_finished": "if [ '${success}' = 'true' ] && [ '${step}' = 'test' ]; then notify-leader; fi"
}
```

Fire-and-forget shell commands on any event. Cascade across pawl instances, notify external systems, trigger downstream workflows.

## More Recipes

### Release Engineering

```json
{
  "workflow": [
    { "name": "bump",    "run": "npm version patch" },
    { "name": "build",   "run": "npm run build", "on_fail": "retry" },
    { "name": "staging", "run": "kubectl apply -f staging.yaml" },
    { "name": "smoke" },
    { "name": "prod",    "run": "kubectl apply -f prod.yaml" },
    { "name": "verify",  "run": "curl -f https://prod/health", "on_fail": "retry" }
  ]
}
```

Deploy to staging → `pawl done` after manual smoke test → deploy to prod → auto-verify.

### Infra with Approval Gate

```json
{
  "vars": { "env": "set -a && source ${project_root}/.env.local && set +a" },
  "workflow": [
    { "name": "plan",   "run": "${env} && terraform plan -out=tfplan" },
    { "name": "review" },
    { "name": "apply",  "run": "${env} && terraform apply tfplan", "on_fail": "manual" },
    { "name": "verify", "run": "${env} && ./smoke-test.sh", "on_fail": "retry" }
  ]
}
```

### Git Worktree Workflow

```json
{
  "vars": {
    "base_branch": "main",
    "branch": "pawl/${task}",
    "worktree": "${project_root}/.pawl/worktrees/${task}"
  },
  "workflow": [
    { "name": "setup",   "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    { "name": "work",    "run": "cd ${worktree} && cat ${task_file} | claude -p",
      "in_viewport": true, "verify": "cd ${worktree} && npm test", "on_fail": "retry" },
    { "name": "review" },
    { "name": "merge",   "run": "cd ${project_root} && git merge --squash ${branch}" },
    { "name": "cleanup", "run": "git worktree remove ${worktree} --force; true" }
  ]
}
```

## Variables

`${var}` in config → expanded before execution. `PAWL_VAR` env vars in subprocesses.

**Intrinsic**: `task` `session` `project_root` `step` `step_index` `log_file` `task_file` `run_id` `retry_count` `last_verify_output`

**User-defined** (`config.vars`): expanded in definition order, can reference earlier vars and intrinsics. Two-layer model: `${var}` expanded by pawl (static, visible in logs), `$ENV_VAR` expanded by shell (dynamic, secrets).

## Output

stdout = JSON, stderr = progress. No `--json` flag — JSON is the only format.

```bash
pawl status task-a | jq .          # routing hints (suggest/prompt)
pawl log task-a --all | jq .       # JSONL event stream
pawl events --follow | jq .        # real-time events
```

## Commands

```bash
pawl init                          # Initialize project
pawl create <name>                 # Create task
pawl start <task> [--reset]        # Start pipeline (--reset = restart)
pawl done <task> [-m msg]          # Approve / mark done
pawl stop <task>                   # Stop
pawl reset <task>                  # Full reset
pawl reset --step <task>           # Retry current step
pawl status [task]                 # Status (JSON)
pawl list                          # All tasks (JSON)
pawl log <task> [--all]            # Logs (JSONL)
pawl events [task] [--follow]      # Event stream
pawl capture <task>                # Viewport content
pawl wait <task> --until <status>  # Block until status
pawl enter <task>                  # Attach to viewport
```

## Design

Three ideas, everything else follows:

1. **`state = replay(log)`** — Append-only JSONL is the single source of truth. Crash, reboot, replay, resume.
2. **Separate what from where** — Recording (what happened) and routing (what to do next) never mix. `decide(Outcome, FailPolicy) → Verdict` is pure.
3. **Trust the substrate** — File system, exit codes, tmux, `grep`. Build only what Unix can't.

## License

MIT
