# Session Handoff

## Current Session (S24): Explicit Plan Step — plan-worker.mjs + ai-helpers.sh extension

### Problem

S24 E2E testing revealed: AI worker spontaneously enters plan mode, `ccc -p` can't approve `ExitPlanMode`, causing zero output. Root cause: plan approval should be an explicit wf workflow step with `verify: "human"` + `wf done`, not an SDK side channel.

### What changed (6 files)

| File | Change |
|------|--------|
| `src/cmd/templates/plan-worker.mjs` | **New**. SDK plan worker using `canUseTool` to intercept `ExitPlanMode` (saves plan to `.wf/plans/${task}.md` + session ID to `.wf/plans/${task}.session`) and auto-answer `AskUserQuestion` |
| `src/cmd/templates/plan-package.json` | **New**. Node.js package.json with `@anthropic-ai/claude-agent-sdk` ^0.2.37 |
| `src/cmd/templates/ai-helpers.sh` | **Modified**. `run_ai_worker()` gains plan-aware branch: detects `.wf/plans/${task}.session` → resumes plan session with `-r` instead of fresh start |
| `src/cmd/init.rs` | **Modified**. Embeds plan-worker.mjs + plan-package.json via `include_str!`, writes them in `create_lib_files()`, adds `node_modules/` to GITIGNORE_ENTRIES |
| `src/cmd/templates/wf-skill.md` | **Modified**. Added Recipe 7: Plan-First Development (plan step → foreman reviews → develop resumes plan session) |
| `.claude/CLAUDE.md` + `README.md` | **Modified**. Updated directory structure to reflect new files |

### SDK verification findings

| Tool | `ccc -p` behavior | SDK `canUseTool` behavior |
|------|-------------------|--------------------------|
| `ExitPlanMode` | **Blocks** (hangs waiting for approval) | Interceptable, get full plan content |
| `AskUserQuestion` | **permission_denied** → AI degrades to text question → exits (no block) | Interceptable, can auto-answer |

Key: `AskUserQuestion` in `ccc -p` returns `is_error: true` with content `"Answer questions?"`. AI sees the error, degrades to text, then exits. No hang, but wasted turn + no actual work done.

### Recipe 7 flow

```
setup → plan (SDK plan mode, verify: human) → develop (resume plan session) → review → merge → cleanup
```

- plan step: `node plan-worker.mjs` → AI explores in read-only plan mode → `ExitPlanMode` intercepted → plan saved → exit 0 → `verify: "human"` → foreman reviews `.wf/plans/${task}.md` → `wf done`
- develop step: `run_ai_worker` detects `.wf/plans/${task}.session` → resumes with `-r session_id` → executes approved plan
- Plan rejection: `wf reset --step` on plan step → re-plans from scratch

### Zero Rust core changes

No changes to: start.rs, run.rs, event.rs, common.rs, config.rs, or any event model. Plan approval uses existing StepCompleted + StepWaiting + StepApproved events.

---

## Previous Sessions (compressed)

### S23: SKILL.md identity fix + E2E test pipeline
- Redefined "AI Agent Orchestrator" → "Resumable Step Sequencer" (6 SKILL.md edits)
- Built E2E test pipeline (Recipe 6 pattern), 4 tests passed 3 design rules

### S22: wf _run replaces runner script + _on-exit
- Single Rust process in tmux: fork/waitpid, SIGHUP immunity, pty safety, race guard

### S20-21: in_window fixes + E2E bootstrap testing

### S13-19: resolve/dispatch refactor + docs restructuring + Skill unification

### S1-12: Architecture evolution + Foreman mode + first principles

---

## Known Issues

- **retry exhaustion has no audit event**: no event emitted when transitioning from retry to terminal state
- `wf events` outputs full history (not filtered by current run), inconsistent with `wf log --all`
- `claude_command` default: projects need `"claude_command": "ccc"` in config for ccc users
- **config validator false positive**: in_window steps using `cd ~/path/${task}` (no worktree) trigger "doesn't reference worktree" warning — validator should check for `cd` not specifically `${worktree}`
- **AskUserQuestion in `ccc -p`**: tool call returns `is_error: true`, AI degrades to text question, wf can't distinguish "AI asked question" from "AI did work but verify failed" — mitigated by plan-first workflow (Recipe 7)

## Key File Index

| Area | File |
|------|------|
| CLI definition (14 commands) | `src/cli.rs` |
| Config model + in_window validation warnings | `src/model/config.rs` |
| Event model + replay + count_auto_retries | `src/model/event.rs` |
| Execution engine + resolve/dispatch | `src/cmd/start.rs` |
| in_window parent process (`wf _run`) | `src/cmd/run.rs` |
| Init (generates SKILL.md + plan-worker + ai-helpers) | `src/cmd/init.rs` |
| Templates (config/skill/ai-helpers/plan-worker) | `src/cmd/templates/` |
| Common utils (event R/W, hooks, check_window_health) | `src/cmd/common.rs` |
| Variables (Context, expand, to_env_vars, claude_command) | `src/util/variable.rs` |
| **Unified Skill reference (~450 lines)** | `src/cmd/templates/wf-skill.md` |
| Project overview | `.claude/CLAUDE.md` |
