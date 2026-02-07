# Session Handoff

## Current Session (S19): Unified SKILL.md + claude_command activation + i18n

### Unified SKILL.md (2 files -> 1 file)

Merged SKILL.md (130 lines) + reference.md (75 lines) into a single SKILL.md (409 lines). Restored critical non-inferrable information that S17 over-deleted:

- **Config Recipes**: 5 verified patterns (basic AI dev / full human review / auto verify+retry / multi-agent parallel+foreman / pure automation)
- **Anti-pattern table**: 4 common misconfigurations with fixes
- **Verify strategy table**: when to use script vs human
- **Foreman coordination**: main loop pseudo-code, status decision table, 3 key constraints
- **AI Worker integration**: run_ai_worker decision flow diagram, parameter table, custom wrapper example
- **Event-variable mapping**: complete 10-event table with extra variables per event
- **Hook concurrency pattern**: mkdir atomic mutex for tmux send-keys

Deleted `reference.md` template and its generation in `init.rs`.

### claude_command activation (was dead field)

`config.claude_command` existed in config.rs but was never wired to anything. Now fully activated:

```
config.jsonc: claude_command → config.rs → Context → ${claude_command} / WF_CLAUDE_COMMAND → ai-helpers.sh
```

- `variable.rs`: Added `claude_command` field to Context, `${claude_command}` expansion, `WF_CLAUDE_COMMAND` env var
- `ai-helpers.sh`: `run_ai_worker` defaults to `${WF_CLAUDE_COMMAND:-claude}` instead of hardcoded `"claude"`
- All `Context::new` call sites updated (common.rs, start.rs x2)

### i18n: Chinese -> English

All project documentation and CLI copy converted to English:
- `src/cmd/templates/wf-skill.md` (full 409-line translation)
- `src/cmd/templates/config.jsonc` (comment)
- `src/cmd/create.rs` (7 template strings in task scaffolding)
- `.wf/config.jsonc` (comment)

### README.md alignment

Config example updated to follow the 3 design rules (was violating all 3). File layout updated to reflect single SKILL.md.

### Build status

36 tests, zero warnings, installed.

---

## Historical Sessions

### S17-18: Skill docs restructuring
- S15-16: docs restructured as `.claude/skills/wf/` skill system
- S17: Skill compression (4 files 949 lines -> 2 files ~200 lines) + config validation warnings
- S18: Identified S17 over-deletion, planned restoration (executed in S19)

### S13-14: resolve/dispatch refactor + E2E
- resolve/dispatch separation, unified WindowLost, wait.rs via Project API, E2E foreman tests

### S9-12: First principles + debate-driven improvements
- Event model audit, step 0-based unification, start --reset, events --follow

### S5-8: Foreman mode
- Non-interactive Claude, wrapper.sh, event hooks, tmux notification loop

### S1-4: Architecture evolution
- TUI removal -> Event Sourcing -> Step model -> Unified Pipeline -> E2E testing

---

## Known Issues

- **on_exit + wf done dual-authority race**: in_window steps have two verdict sources that can fire simultaneously
- **on_exit loses RunOutput**: in_window process exit has no stdout/stderr/duration
- **retry exhaustion has no audit event**: no event emitted when transitioning from retry to terminal state
- `wf events` outputs full history (not filtered by current run), inconsistent with `wf log --all`

## Key File Index

| Area | File |
|------|------|
| CLI definition (14 commands) | `src/cli.rs` |
| Config model + **in_window validation warnings** | `src/model/config.rs` |
| Event model + replay + count_auto_retries | `src/model/event.rs` |
| Execution engine + resolve/dispatch pipeline | `src/cmd/start.rs` |
| Init (generates single SKILL.md) | `src/cmd/init.rs` |
| Templates (config/skill/ai-helpers) | `src/cmd/templates/` |
| Common utils (event R/W, hooks, check_window_health) | `src/cmd/common.rs` |
| Variables (Context, expand, to_env_vars, **claude_command**) | `src/util/variable.rs` |
| Unified Skill reference (409 lines) | `src/cmd/templates/wf-skill.md` |
| Project overview | `.claude/CLAUDE.md` |
