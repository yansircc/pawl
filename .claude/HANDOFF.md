# Session Handoff

## Current Session (S39): E2E viewport tests, doc fix, log cleanup

### What changed

**E2E viewport 测试 — 从 0 到 32 个测试覆盖所有 in_viewport 路径**

测试:
- 新增 `tests/e2e-viewport.sh`: 32 个 viewport E2E 测试，并行执行 (~5s)
- 新增 `tests/e2e.sh`: 72 个同步路径 E2E 测试（之前在 working dir 未提交）
- 覆盖: 基本流程、done 外部完成、retry、viewport loss、capture、enter、stop、变量、连续 in_viewport exec 链、done -m、skip viewport 步骤、viewport hooks、verify fail + retry、task 索引解析、full reset with viewport、多 task 并发、events --follow、last_feedback 传递
- Session 隔离: 每个测试用唯一 tmux session `pawl-e2e-vp-{name}`，trap EXIT 清理

文档 — viewport_lost 语义澄清:
- **CLAUDE.md**: `viewport_lost` 从 "viewport disappeared" 改为安全网语义；`_run` 加注韧性模型；`detect_viewport_loss` 加注被动前提
- **orchestrate.md**: event hooks 列表内联安全网说明
- **supervise.md**: viewport failure 两条路径模型（正常 path = `step_finished(128)`，安全网 = `viewport_lost`）

代码:
- 删除 `pawl log --all-runs` flag（死代码，`pawl events` 已覆盖 cross-run 需求）
- CLAUDE.md 中 log 命令描述同步更新

关键发现:
- `_run` 韧性（SIGHUP-immune, always settles）导致 viewport kill 的正常路径是 `step_finished(exit_code=128)` 而非 `viewport_lost`
- `viewport_lost` 仅当 `_run` 本身被 SIGKILL/crash 时触发（安全网）
- 这是文档缺失，不是设计缺陷——两条路径收敛到同一终态 `Failed`

---

## Previous Sessions (compressed)

### S38: Decouple from git, add config.vars
- `git.rs` → `project.rs`，`get_project_root()` 查找 `.pawl/`
- Config: 删 `worktree_dir`/`base_branch`，新增 `vars: IndexMap`
- `pawl init` 不依赖 git。Git worktree 降为 recipe

### S37: Skill Self-Containment + human→manual
- Skill 文档 zero-jump 自完备
- config.jsonc: 46→4 行（空画布）
- `"human"` → `"manual"` 跨 13 文件

### S36: Less-Is-More Audit
- 三生成元 + less-is-more 停机条件
- PawlError 精简。derive_routing() 移到 status.rs

### S33-S35: Agent-First Interface
- stdout=JSON, stderr=progress。exit codes 2-7。derive_routing() 自路由。

### S32 and earlier
- S32: Role-based skill architecture
- S30: 解耦 Claude Code
- S28: settle_step pipeline, Display+FromStr, context_for
- S27: Viewport trait + TmuxViewport
- S25-26: Rename wf → pawl, crates.io
- S1-24: Architecture evolution

---

## Pending Work

None.

## Known Issues

- **retry exhaustion has no audit event**: no event when transitioning from retry to terminal state
- `pawl events` outputs full history (not filtered by current run), inconsistent with `pawl log --all`

## Key File Index

| Area | File |
|------|------|
| CLI definition (14 commands) | `src/cli.rs` |
| PawlError enum (6 variants, exit codes 2-7) | `src/error.rs` |
| Project context, context_for, output_task_state | `src/cmd/common.rs` |
| Status + derive_routing (suggest/prompt) | `src/cmd/status.rs` |
| Execution engine, settle_step, decide() | `src/cmd/start.rs` |
| in_viewport parent process (`pawl _run`) | `src/cmd/run.rs` |
| Done/approve handler | `src/cmd/done.rs` |
| Wait (poll with Timeout) | `src/cmd/wait.rs` |
| Entry point, PawlError → text stderr | `src/main.rs` |
| Project root discovery, task name validation | `src/util/project.rs` |
| Context builder (expand/to_env_vars/var_owned) | `src/util/variable.rs` |
| Shell command execution | `src/util/shell.rs` |
| Config model + Step + vars (IndexMap) | `src/model/config.rs` |
| Event model + replay + count_auto_retries | `src/model/event.rs` |
| TaskState, TaskStatus, StepStatus | `src/model/state.rs` |
| Templates (config + skill + references) | `src/cmd/templates/` |
| Viewport trait + TmuxViewport | `src/viewport/` |
| E2E tests (sync paths, 72 tests) | `tests/e2e.sh` |
| E2E tests (viewport paths, 32 tests) | `tests/e2e-viewport.sh` |
