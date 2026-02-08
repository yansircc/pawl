# Session Handoff

## Current Session (S42): Skill doc stress test + viewport close bug fix

### What changed

**1. `done.rs` viewport close 时序 bug fix**

当 `pawl done` 把一个 in_viewport 步骤推进到下一个 in_viewport 步骤时，旧代码先调 `resume_workflow()`（为新步骤打开 viewport），再调 `viewport.close()`（杀掉 viewport）。结果把刚打开的新步骤 viewport 杀了，导致 viewport_lost。

修复：viewport close 移到 resume_workflow 之前。单元测试通过，E2E viewport 测试待跑。

**2. `supervise.md` Monitoring 重排**

原文档把 Poll 排第一标 "(default)"，导致 agent 锚定到 sleep+list 模式。重排为：
- Wait (preferred) — 多任务并行 `pawl wait & wait` 模式，带完整示例
- Events (real-time) — `pawl events --follow --type` 作为 live dashboard
- Poll (fallback) — 明确标注仅在 wait/events 不可行时使用

**3. Skill 文档质量压测**

用 pawl 编排工作流，5 个不同主题（C/Go/Python/Web/Infra）的 agent 并行，仅凭 skill 文档冷启动产出 config.jsonc + tasks。结果：
- 5/5 config 格式正确，能被 pawl list 解析
- 5/5 都用了 git worktree skeleton recipe（锚定效应过强）
- verify 策略按领域自适应（C→make, Go→build+vet, Python→pytest, Infra/Web→manual）
- 一个质量 bug：Python config 的 verify 写了 `|| true` 导致永远通过

---

## Previous Sessions (compressed)

### S41: Agent E2E tests
- `tests/e2e-agent.sh`: 9 个 agent E2E 测试（真实 haiku agent 与 pawl 交互）
- 3 种 agent 角色: supervisor, worker, verifier

### S39: E2E viewport tests, doc fix, log cleanup
- `tests/e2e-viewport.sh`: 32 viewport E2E (并行)
- `tests/e2e.sh`: 72 sync E2E
- `viewport_lost` 安全网语义文档修正

### S38: Decouple from git, add config.vars
- `git.rs` → `project.rs`，`get_project_root()` 查找 `.pawl/`
- Config: 删 `worktree_dir`/`base_branch`，新增 `vars: IndexMap`

### S37: Skill Self-Containment + human→manual
- Skill 文档 zero-jump 自完备
- `"human"` → `"manual"` 跨 13 文件

### S36 and earlier
- S36: Less-Is-More Audit
- S33-S35: Agent-First Interface (stdout=JSON, exit codes, derive_routing)
- S32: Role-based skill architecture
- S30: 解耦 Claude Code
- S28: settle_step pipeline, Display+FromStr, context_for
- S27: Viewport trait + TmuxViewport

---

## Pending Work

- **E2E viewport 测试验证 done.rs 修复**：`tests/e2e-viewport.sh` 需要跑一次确认无回归
- **orchestrate.md Plain Workflow recipe**：当前只有 git worktree skeleton，导致 5/5 agent 锚定到 worktree 模式。需要在 worktree recipe 之前加一个无 git 的 Plain 模式 recipe + 决策指引

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
