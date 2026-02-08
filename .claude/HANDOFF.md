# Session Handoff

## Current Session (S38): Decouple from git, add config.vars, reposition

### What changed

**pawl 解耦 git — 从 "git worktree orchestrator" 变为通用步骤序列器**

代码层:
- `git.rs` → `project.rs`: `get_project_root()` 向上查找 `.pawl/`，`validate_task_name()` 纯文件系统检查
- Config: 删 `worktree_dir`/`base_branch`，新增 `vars: IndexMap<String, String>`（insertion-order）
- `context_for()`: 内置变量 + config.vars 按序展开（earlier available to later）
- `var_owned(String, String)` on Context 支持动态 key
- Project field `repo_root` → `project_root`
- `pawl init` 用 cwd（不依赖 git），`.gitignore` 更新 best-effort
- 删除: `branch_exists()`, `get_repo_root()`, `run_command_output()`, `run_command_with_options()` dir 参数, config worktree 警告, control.rs git 清理提示
- Git worktree recipe 移至 orchestrate.md 的 `config.vars` 示例

文档层 — 重新定位:
- **README.md**: 从 "agent-friendly step sequencer" → "shell's missing yield"。开头用通用例子（build/test/deploy），Agent Orchestration 作为首要 showcase（自路由、递归 supervisor tree），git worktree 降为 recipe
- **CLAUDE.md**: 开头从 "Agent-Friendly Resumable Step Sequencer" → "Durable Execution Primitive for Shell"
- **orchestrate.md**: 新增 User Variables 章节 + `.env` secrets recipe + 更新 git worktree recipe 用 config.vars

Greenfield 自查:
- `run_command_with_options` 死参数 `dir` 清理（内联到两个调用方）
- 零 warning，38 tests，无禁止模式

---

## Previous Sessions (compressed)

### S37: Skill Self-Containment + human→manual
- Skill 文档 zero-jump 自完备
- config.jsonc: 46→4 行（空画布）
- CLI 8 个 `after_help` 全删
- `"human"` → `"manual"` 跨 13 文件

### S36: Less-Is-More Audit
- 三生成元 + less-is-more 停机条件
- PawlError 精简。derive_routing() 移到 status.rs

### S33-S35: Agent-First Interface
- stdout=JSON, stderr=progress。exit codes 2-7。derive_routing() 自路由。

### S32 and earlier
- S32: SKILL.md 249→29 行。Role-based skill architecture。
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
