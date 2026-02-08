# Session Handoff

## Current Session (S41): Agent E2E tests

### What changed

**Agent E2E 测试 — 9 个真实 haiku agent 与 pawl 交互的测试**

新增 `tests/e2e-agent.sh`: 9 个 agent E2E 测试，并行执行 (~60s, ~$0.05/run)

三组测试:
- **Supervisor 路由 (4)**: gate→done, verify_manual→done, on_fail→reset --step, multi-step loop — agent 读 `pawl status` 路由提示，执行正确命令
- **Worker + Verifier (3)**: viewport agent 创建文件, verifier pass, verifier fail→retry→pass — agent 作为 viewport worker 和 structured output verifier
- **反馈循环 (2)**: worker 写 INITIAL → verifier reject → worker 读 `$PAWL_LAST_VERIFY_OUTPUT` → 写 CORRECTED → pass; multi-task done

三种 agent 角色:
| 角色 | `--tools` | 用途 |
|------|-----------|------|
| Supervisor | `"Bash"` | 跑 `pawl status`/`done`/`reset` |
| Worker | `"Bash"` | 在 viewport 中执行任务 |
| Verifier | `"Bash"` + `--json-schema` | 检查产出，返回 `{pass: bool}` |

关键发现:
- ccc Bash tool **会重置 cwd** → prompt 必须包含 `cd <project_dir>`
- macOS 无 `timeout` → `run_with_timeout()` (background + watchdog kill)
- 不能从 Claude Code Bash tool 嵌套调用 ccc（会挂住），必须独立脚本
- 所有 agent 调用: `--output-format stream-json --verbose --max-budget-usd 0.02`

---

## Previous Sessions (compressed)

### S39: E2E viewport tests, doc fix, log cleanup
- `tests/e2e-viewport.sh`: 32 viewport E2E (并行)
- `tests/e2e.sh`: 72 sync E2E
- `viewport_lost` 安全网语义文档修正
- 删除 `pawl log --all-runs`

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
| E2E tests (agent paths, 9 tests) | `tests/e2e-agent.sh` |
