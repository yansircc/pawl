# Session Handoff

## Current Session (S22): `wf _run` replaces runner script + `_on-exit`

### What changed

Replaced the entire bash indirect chain (runner script → env export → EXIT trap → `wf _on-exit`) with a single Rust process `wf _run task step` that runs inside the tmux pane.

**Before**: `execute_in_window()` wrote a bash script with env exports + HUP trap + EXIT trap calling `wf _on-exit`, sent `bash 'script'` to tmux. S21 fixed 3 bugs in this chain (HUP trap, $? clobber, pty panic). Debate then found more issues (race conditions, cd short-circuit).

**After**: `execute_in_window()` sends `wf _run task step_idx` to tmux. `wf _run` is a Rust process that:
1. Ignores SIGHUP (survives tmux kill-window)
2. Forks child via `bash -c` with `pre_exec(SIG_DFL)` (child receives signals normally)
3. `waitpid` for child exit
4. Redirects stdout/stderr → /dev/null (pty may be gone)
5. Re-checks state (step_idx guard against `wf done` race)
6. Calls `handle_step_completion()` → unified pipeline

### Files changed

| File | Change |
|------|--------|
| `Cargo.toml` | +`libc = "0.2"` |
| `src/cli.rs` | `_on-exit` → `_run` (replaced, not added alongside) |
| `src/cmd/run.rs` | **New** — 142 lines, core `_run` implementation |
| `src/cmd/mod.rs` | +`pub mod run;`, dispatch `Run` variant |
| `src/cmd/start.rs` | `execute_in_window()` rewritten: deleted runner script generation (~46 lines), replaced with `wf _run` send (~25 lines). Added `WF_RUNNING_IN_WINDOW` exec optimization for consecutive in_window steps. |
| `src/cmd/control.rs` | **Deleted** `on_exit()` function + cleaned imports |
| `.claude/CLAUDE.md` | Updated architecture docs |

### Issues resolved by this change

- ~~on_exit + wf done dual-authority race~~: `_run` re-checks `status==Running && current_step==step_idx` after waitpid
- ~~on_exit loses RunOutput~~: same as before (no stdout/stderr/duration for in_window), but now structurally equivalent — no bash intermediary to lose data
- ~~in_window log system blind spot~~: `_run` is a Rust process, event emission is guaranteed (not best-effort bash trap)
- ~~_on-exit trap 3 bugs (S21)~~: entire chain eliminated

### E2E verification

| Scenario | Result |
|----------|--------|
| Normal in_window completion | exit_code=0 → Advance → continue_execution → Completed |
| in_window failure (exit 42) | exit_code=42 captured → Failed |
| tmux kill-window | child killed by HUP, parent survives → exit_code=128 → Failed |
| wf done race (done before _run finishes) | done advances step, _run re-checks and skips — no duplicate events |

---

## Previous Sessions (compressed)

### S21: _on-exit trap 3-bug fix
- Root cause: bash SIG_DFL(HUP) skips EXIT trap. Fixed HUP trap + $? clobber + pty panic.
- **Superseded by S22** — entire bash chain eliminated.

### S20: in_window fixes + E2E bootstrap testing
- Runner script pattern, ai-helpers.sh grep safety, SKILL.md bootstrap testing (3 projects)

### S19: Unified SKILL.md + claude_command activation + i18n

### S13-18: resolve/dispatch refactor + docs restructuring

### S9-12: First principles + debate-driven improvements

### S1-8: Architecture evolution + Foreman mode

---

## Known Issues

- **retry exhaustion has no audit event**: no event emitted when transitioning from retry to terminal state
- `wf events` outputs full history (not filtered by current run), inconsistent with `wf log --all`
- `claude_command` default: projects need `"claude_command": "ccc"` in config for ccc users

## Key File Index

| Area | File |
|------|------|
| CLI definition (14 commands) | `src/cli.rs` |
| Config model + in_window validation warnings | `src/model/config.rs` |
| Event model + replay + count_auto_retries | `src/model/event.rs` |
| Execution engine + resolve/dispatch | `src/cmd/start.rs` |
| **in_window parent process (`wf _run`)** | `src/cmd/run.rs` |
| Init (generates single SKILL.md) | `src/cmd/init.rs` |
| Templates (config/skill/ai-helpers) | `src/cmd/templates/` |
| Common utils (event R/W, hooks, check_window_health) | `src/cmd/common.rs` |
| Variables (Context, expand, to_env_vars, claude_command) | `src/util/variable.rs` |
| Unified Skill reference (409 lines) | `src/cmd/templates/wf-skill.md` |
| Project overview | `.claude/CLAUDE.md` |
