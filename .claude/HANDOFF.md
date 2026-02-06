# Session Handoff

## 本次 Session 完成的工作

### Unified Step Pipeline 重构

统一步骤管线重构，消除 on_exit 竞态复杂度，精简事件和命令。

**Event 模型 14→11**
- 删除: `CommandExecuted`, `OnExit`, `AgentReported`, `StepRetried`, `StepRolledBack`, `AgentResult` enum
- 新增: `StepCompleted { ts, step, exit_code, duration?, stdout?, stderr? }` — 统一替代同步执行/on_exit/agent done
- 新增: `StepReset { ts, step, auto: bool }` — 替代 StepRetried + StepRolledBack

**统一管线**
- `handle_step_completion()` — 单一入口处理 verify + on_fail
- `apply_on_fail()` — 替代散布的 `handle_verify_failure()`
- `on_exit()` 从 93 行简化到 47 行，无竞态处理
- `VerifyOutcome` 删除 `HumanRequired`，verify:"human" 通过 Failed{feedback:""} 路由到 apply_on_fail

**CLI 命令 19→13**
- 删除: `wf next`, `wf retry`, `wf back`, `wf skip`, `wf fail`
- 修改: `wf reset <task> [--step]` — `--step` 做步骤重试（替代 `wf retry`）
- `agent.rs` → `approve.rs`，只保留 `done()`

**TaskDefinition 新增 skip**
- `skip: Vec<String>` — 按步骤名自动跳过
- 支持 YAML 列表和内联数组格式

**Greenfield 清理**
- 删除 `extract_session_id()` 和 `get_transcript_path()` dead code（不用 `#[allow(dead_code)]`）

**净变更**: ~570 行净删除，27 测试全部通过

---

## 历史 Session

### Session 2: Step 验证模型改造
- Step 从 checkpoint/block 体系迁移到 4 正交属性（run, verify, on_fail, in_window）
- 事件重命名: CheckpointReached → StepWaiting, CheckpointPassed → StepApproved
- 新增 VerifyFailed 事件，verify helpers（run_verify, handle_verify_failure, count_verify_failures）

### Session 1: TUI 删除 + Event Sourcing
- 删除 TUI 模块（-3,372 行）
- 建立 Event Sourcing 架构（JSONL 单一事实来源）

---

## 关键文件索引

| 功能 | 文件 |
|------|------|
| CLI 定义（13 命令） | `src/cli.rs` |
| 配置模型（Step 4 属性） | `src/model/config.rs` |
| 事件模型 + replay（11 种） | `src/model/event.rs` |
| 状态投影类型 | `src/model/state.rs` |
| 任务定义（含 skip） | `src/model/task.rs` |
| 执行引擎 + 统一管线 | `src/cmd/start.rs` |
| 审批命令（done） | `src/cmd/approve.rs` |
| 控制命令（stop/reset/on_exit） | `src/cmd/control.rs` |
| 公共工具（事件读写） | `src/cmd/common.rs` |
| 项目概述 | `.claude/CLAUDE.md` |
