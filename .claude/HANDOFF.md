# Session Handoff

## Current Session (S20): in_window bug fixes + E2E bootstrap testing

### Bug fixes (2 commits)

#### 1. Runner script for in_window execution (`start.rs`)

**Problem**: `execute_in_window()` sent commands directly via `tmux send-keys`. Two issues:
- WF_* env vars were never injected into tmux window (AI worker crashed on unset `$WF_LOG_FILE`)
- tmux shell is zsh, but `ai-helpers.sh` is bash — `source` into zsh caused compatibility issues

**Fix**: Write a bash runner script to `.wf/logs/.run.{task}.sh` containing env exports + trap + command, then send `bash 'script'` to tmux. This guarantees bash execution regardless of the user's shell.

**File**: `src/cmd/start.rs` — `execute_in_window()` function

#### 2. ai-helpers.sh grep safety

**Problem**: `set -euo pipefail` + `grep` finding no matches (exit 1) = shell exit. Fresh JSONL files have no `session_id` entries, so `extract_session_id()` always killed the shell.

**Fix**: Changed `set -euo pipefail` → `set -uo pipefail` (removed `-e`). Wrapped all `grep` calls with `{ grep ... || true; }`.

**File**: `src/cmd/templates/ai-helpers.sh`

### E2E bootstrap testing

Tested SKILL.md's ability to guide AI in configuring wf for new projects:

**Test 1: nanobot-go** (Go project, detailed prompt with hints)
- AI correctly produced config with `go test ./...` verify, proper Design Rules compliance
- 5 tasks with dependency chain: store/provider/workspace/tools → agent

**Test 2: bm** (Go project, minimal prompt — one sentence)
- Prompt: `"为这个项目配置 wf 工作流并创建开发任务。"`
- AI correctly inferred `go build ./... && go test ./...` for verify
- 3 tasks with dependencies: db → fetch → cli

**Test 3: pv** (Python project, minimal prompt — one sentence)
- Prompt: `"做一个类似 dpaste 的命令行粘贴板工具，Python 实现。写好 PLAN.md，配置 wf 工作流，创建开发任务。"`
- AI correctly inferred `python -m pytest tests/ -q` for verify
- Wrote its own PLAN.md + 4 tasks: core → api → cli → polish

**Conclusion**: SKILL.md successfully guides AI to produce correct configs and tasks from one-sentence prompts. 3 Config Design Rules are consistently followed.

### Remaining work for next session

- **jql (Rust) and kv (Node.js) bootstrap**: not yet tested (tmux sessions cleaned up before completion)
- **`_on-exit` trap not triggering in tmux**: after AI worker finishes, `wf _on-exit` should fire via bash EXIT trap, but status remains "running". Needs investigation — may be a tmux + bash interaction issue
- **`claude_command` default**: projects need `"claude_command": "ccc"` in config for ccc users. Default template still has `"claude"`. Consider auto-detecting or documenting this better
- **wf as foreman workflow**: user wants to build a standardized workflow where wf manages multiple AI agents across projects. The bootstrap pattern (one-sentence prompt → AI configures everything) works but needs polish

### Test projects created (in ~/code/tmp/)

| Project | Tech | Status | Notes |
|---------|------|--------|-------|
| nanobot-go | Go | bootstrap done | 5 tasks, deps correct |
| bm | Go | bootstrap done | 3 tasks, minimal prompt |
| pv | Python | bootstrap done | 4 tasks, AI wrote PLAN.md |
| jql | Rust | not tested | wf init done, bootstrap task created |
| kv | Node.js | not tested | wf init done, bootstrap task created |

---

## Historical Sessions

### S19: Unified SKILL.md + claude_command activation + i18n
- 2→1 file SKILL.md (409 lines), claude_command wired through, English i18n

### S13-18: resolve/dispatch refactor + docs restructuring
- resolve/dispatch separation, WindowLost unification, wait.rs via Project API
- Skill docs: 4 files → 2 files → 1 file, config validation warnings

### S9-12: First principles + debate-driven improvements
- Event model audit, step 0-based unification, start --reset, events --follow

### S1-8: Architecture evolution + Foreman mode
- TUI removal → Event Sourcing → Step model → Foreman → E2E testing

---

## Known Issues

- **_on-exit trap not triggering in tmux**: bash EXIT trap in runner script may not fire correctly when tmux window's bash process exits — needs investigation
- **on_exit + wf done dual-authority race**: in_window steps have two verdict sources that can fire simultaneously
- **on_exit loses RunOutput**: in_window process exit has no stdout/stderr/duration
- **retry exhaustion has no audit event**: no event emitted when transitioning from retry to terminal state
- `wf events` outputs full history (not filtered by current run), inconsistent with `wf log --all`

## Key File Index

| Area | File |
|------|------|
| CLI definition (14 commands) | `src/cli.rs` |
| Config model + in_window validation warnings | `src/model/config.rs` |
| Event model + replay + count_auto_retries | `src/model/event.rs` |
| Execution engine + **runner script** + resolve/dispatch | `src/cmd/start.rs` |
| Init (generates single SKILL.md) | `src/cmd/init.rs` |
| Templates (config/skill/**ai-helpers**) | `src/cmd/templates/` |
| Common utils (event R/W, hooks, check_window_health) | `src/cmd/common.rs` |
| Variables (Context, expand, to_env_vars, claude_command) | `src/util/variable.rs` |
| Unified Skill reference (409 lines) | `src/cmd/templates/wf-skill.md` |
| Project overview | `.claude/CLAUDE.md` |
