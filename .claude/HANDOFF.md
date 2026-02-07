# Session Handoff

## Current Session (S23): SKILL.md identity fix + E2E test pipeline

### What changed

1. **Redefined wf identity**: "AI Agent Orchestrator" → "Resumable Step Sequencer". SKILL.md and CLAUDE.md anchored all readers (AI and human) to one use case (AI coding with git worktrees), causing agents to miss wf's generic step sequencer capability.

2. **Built E2E test pipeline**: Rewrote `.wf/config.jsonc` as a generic pipeline (no worktrees) to test SKILL.md's information completeness. Each task = one test scenario with different prompt specificity levels.

### SKILL.md changes (6 edits)

| Edit | What | Why |
|------|------|-----|
| Title | "AI Agent Orchestrator" → "Resumable Step Sequencer" | Break first-frame anchoring |
| Opening | Remove "AI coding agents" / "git worktree" identity | Generalize identity |
| Rule 3 | `cd ${worktree}` → `cd to the working directory` | Don't bind rules to worktree |
| Exception clause | Added after rules: utility steps may omit verify | Resolve rules vs recipe contradiction |
| Anti-pattern table | Generalized `cd` example | Consistency |
| Recipe 6 | New: Generic Pipeline (No Git Worktrees) | Break 5/5 coding-only recipe anchoring |
| AI Worker section title | Added "(Coding Workflow Pattern)" framing | 63 lines were unframed as one pattern |

### E2E test pipeline

Rewrote `.wf/config.jsonc` for testing (Recipe 6 pattern — no worktrees):

```
setup → init-wf → bootstrap (AI in tmux) → review (gate)
```

Each task's body = project PLAN + AI instructions, piped to `ccc -p`. 4 test tasks created:

| Task | Prompt level | Result |
|------|-------------|--------|
| bm-explicit | Explicit: "read SKILL.md" | ✅ 3/3 rules, 2 tasks, verify=go build |
| bm-wf-only | Moderate: "use wf to configure" | ✅ 3/3 rules, 3 tasks, verify=go build |
| bm-vague | Extreme: "configure dev workflow" (no wf mention) | ✅ 3/3 rules, 4 tasks, verify=go build+vet |
| todo-py | Different language: Python project | ✅ 3/3 rules, 1 task, verify=python todo.py ls |

**Key finding**: All 4 tests pass 3 design rules. Claude Code's skill auto-discovery ensures AI reads SKILL.md even without explicit mention. Only `claude_command` needs explicit instruction.

### Root cause analysis

The session started with repeated miscommunication about "use wf to orchestrate testing." The foreman (me) kept interpreting "use wf" as "set up wf development workflow in target project" instead of "use wf as a generic pipeline for your own work." Root cause: **I conflated wf's instance (coding workflow template) with wf's type (generic step sequencer)** — the exact anchoring problem SKILL.md was creating for all readers.

### Files changed

| File | Change |
|------|--------|
| `src/cmd/templates/wf-skill.md` | Identity fix: title, opening, rule 3, exception, anti-pattern, Recipe 6, AI Worker framing |
| `.claude/CLAUDE.md` | Title + opening aligned with SKILL.md |
| `.wf/config.jsonc` | Rewritten as E2E test pipeline (Recipe 6 pattern) |
| `.wf/tasks/*.md` | 4 test task definitions (bm-explicit, bm-wf-only, bm-vague, todo-py) |

---

## Previous Sessions (compressed)

### S22: wf _run replaces runner script + _on-exit
- Eliminated entire bash indirect chain (runner script → env → trap → wf _on-exit)
- Single Rust process `wf _run task step` in tmux: fork/waitpid, SIGHUP immunity, pty safety, race guard

### S20-21: in_window fixes + E2E bootstrap testing
- S21: 3 trap bugs (superseded by S22). S20: ai-helpers.sh safety, SKILL.md bootstrap testing

### S13-19: resolve/dispatch refactor + docs restructuring + Skill unification

### S1-12: Architecture evolution + Foreman mode + first principles

---

## Known Issues

- **retry exhaustion has no audit event**: no event emitted when transitioning from retry to terminal state
- `wf events` outputs full history (not filtered by current run), inconsistent with `wf log --all`
- `claude_command` default: projects need `"claude_command": "ccc"` in config for ccc users
- **config validator false positive**: in_window steps using `cd ~/path/${task}` (no worktree) trigger "doesn't reference worktree" warning — validator should check for `cd` not specifically `${worktree}`

## Key File Index

| Area | File |
|------|------|
| CLI definition (14 commands) | `src/cli.rs` |
| Config model + in_window validation warnings | `src/model/config.rs` |
| Event model + replay + count_auto_retries | `src/model/event.rs` |
| Execution engine + resolve/dispatch | `src/cmd/start.rs` |
| in_window parent process (`wf _run`) | `src/cmd/run.rs` |
| Init (generates single SKILL.md) | `src/cmd/init.rs` |
| Templates (config/skill/ai-helpers) | `src/cmd/templates/` |
| Common utils (event R/W, hooks, check_window_health) | `src/cmd/common.rs` |
| Variables (Context, expand, to_env_vars, claude_command) | `src/util/variable.rs` |
| **Unified Skill reference (~430 lines)** | `src/cmd/templates/wf-skill.md` |
| Project overview | `.claude/CLAUDE.md` |
