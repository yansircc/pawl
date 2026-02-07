# Session Handoff

## Current Session (S25): Rename wf → pawl

### What changed

Full product rename from `wf` to `pawl`. Binary, directories, environment variables, branch prefixes, all user-facing strings, templates, and documentation.

### Scope (19 source files + 4 templates + 3 docs)

| Category | Change |
|----------|--------|
| **Binary name** | `Cargo.toml` name = "pawl", `cli.rs` command name = "pawl" |
| **Data directory** | `.wf/` → `.pawl/` (constants in init.rs, common.rs, create.rs) |
| **Environment variables** | `WF_*` → `PAWL_*` (13 variables in variable.rs, run.rs, start.rs) |
| **Branch prefix** | `wf/{task}` → `pawl/{task}` (variable.rs, control.rs, git.rs) |
| **CLI messages** | All `'wf start'` → `'pawl start'` etc. across 7 cmd files |
| **Templates** | config.jsonc, ai-helpers.sh, plan-worker.mjs fully updated |
| **Skill file** | `wf-skill.md` → `pawl-skill.md` (renamed + ~80 internal replacements) |
| **Skill directory** | `.claude/skills/wf/` → `.claude/skills/pawl/` |
| **Documentation** | README.md, CLAUDE.md, HANDOFF.md fully updated |

### Verification

- **Build**: zero warnings
- **Tests**: 36/36 passed
- **Install**: `~/.cargo/bin/pawl` installed, `pawl --version` returns `pawl 0.1.0`

### Migration notes for existing projects

- `mv .wf .pawl` to migrate data directory
- Update `.gitignore` entries from `.wf/` to `.pawl/`
- Clean up orphaned `wf/*` git branches: `git branch -l 'wf/*' | xargs git branch -D`
- Skill directory: `mv .claude/skills/wf .claude/skills/pawl`

---

## Previous Sessions (compressed)

### S24: Explicit Plan Step — plan-worker.mjs
- SDK plan worker with `canUseTool` interception for `ExitPlanMode`
- ai-helpers.sh plan-aware resume branch
- Recipe 7: Plan-First Development flow

### S23: SKILL.md identity fix + E2E test pipeline
- Redefined "AI Agent Orchestrator" → "Resumable Step Sequencer"
- Built E2E test pipeline (Recipe 6 pattern)

### S22: pawl _run replaces runner script + _on-exit
- Single Rust process in tmux: fork/waitpid, SIGHUP immunity, pty safety, race guard

### S20-21: in_window fixes + E2E bootstrap testing

### S13-19: resolve/dispatch refactor + docs restructuring + Skill unification

### S1-12: Architecture evolution + Foreman mode + first principles

---

## Known Issues

- **retry exhaustion has no audit event**: no event emitted when transitioning from retry to terminal state
- `pawl events` outputs full history (not filtered by current run), inconsistent with `pawl log --all`
- `claude_command` default: projects need `"claude_command": "ccc"` in config for ccc users
- **config validator false positive**: in_window steps using `cd ~/path/${task}` (no worktree) trigger "doesn't reference worktree" warning — validator should check for `cd` not specifically `${worktree}`
- **AskUserQuestion in `ccc -p`**: tool call returns `is_error: true`, AI degrades to text question, pawl can't distinguish "AI asked question" from "AI did work but verify failed" — mitigated by plan-first workflow (Recipe 7)

## Key File Index

| Area | File |
|------|------|
| CLI definition (14 commands) | `src/cli.rs` |
| Config model + in_window validation warnings | `src/model/config.rs` |
| Event model + replay + count_auto_retries | `src/model/event.rs` |
| Execution engine + resolve/dispatch | `src/cmd/start.rs` |
| in_window parent process (`pawl _run`) | `src/cmd/run.rs` |
| Init (generates SKILL.md + plan-worker + ai-helpers) | `src/cmd/init.rs` |
| Templates (config/skill/ai-helpers/plan-worker) | `src/cmd/templates/` |
| Common utils (event R/W, hooks, check_window_health) | `src/cmd/common.rs` |
| Variables (Context, expand, to_env_vars, claude_command) | `src/util/variable.rs` |
| **Unified Skill reference (~450 lines)** | `src/cmd/templates/pawl-skill.md` |
| Project overview | `.claude/CLAUDE.md` |
