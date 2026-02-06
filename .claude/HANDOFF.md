# Session Handoff

## 本次 Session 完成的工作

### Step 验证模型改造

将 Step 模型从 checkpoint/block 体系迁移到 4 个正交属性（`run`, `verify`, `on_fail`, `in_window`），消除了 checkpoint 和 block 概念。

**Phase 1: 删除 block 概念**
- 删除 `AgentResult::Blocked`、`StepStatus::Blocked`、`wf block` CLI 命令
- 清理 TUI 中所有 Blocked 引用（8 个文件）

**Phase 2: 配置模型扩展**
- Step 新增 `on_fail`（"retry"/"human"）和 `max_retries` 字段
- 新增 `is_gate()`、`verify_is_human()`、`on_fail_retry()`、`on_fail_human()`、`effective_max_retries()` 方法
- `is_checkpoint()` 删除

**Phase 3: 事件模型改造**
- `CheckpointReached` → `StepWaiting`，`CheckpointPassed` → `StepApproved`
- 新增 `VerifyFailed` 事件（含 `feedback` 字段）
- 14 种事件类型，更新 replay/type_name/step_index/extra_vars

**Phase 4: 执行引擎改造**
- `start.rs`：新增 `run_verify()`、`handle_verify_failure()`、`count_verify_failures()`
- 执行循环支持 gate+human verify、普通步骤后 verify、on_fail 策略（auto-retry/human/default）
- `control.rs`：`on_exit()` 集成 verify 逻辑
- `agent.rs`：`done()`/`fail()` 扩展支持 Waiting 状态

**Phase 5+6: 显示层 + 文档**
- TUI：`StepType::Checkpoint` → `StepType::HumanVerify`
- init.rs：DEFAULT_CONFIG 使用 `verify: "human"` 替代空 checkpoint
- 更新 CLAUDE.md、.wf/README.md

**Greenfield 清理**
- 删除过时的 `docs/` 目录（10 个文件）
- 删除已完成的 spec、过时的 HANDOFF.md 和 insights.md

**改动文件汇总**: 22 个 Rust 源文件修改，14 个文档文件删除/更新

---

## 关键文件索引

| 功能 | 文件 |
|------|------|
| CLI 定义 | `src/cli.rs` |
| 配置模型（Step 4 属性） | `src/model/config.rs` |
| 事件模型 + replay（14 种） | `src/model/event.rs` |
| 状态投影类型 | `src/model/state.rs` |
| 执行引擎 + verify helpers | `src/cmd/start.rs` |
| Agent 命令（done/fail） | `src/cmd/agent.rs` |
| 控制命令（next/retry/on_exit） | `src/cmd/control.rs` |
| 公共工具（事件读写） | `src/cmd/common.rs` |
| 项目概述 | `.claude/CLAUDE.md` |
| 配置指南 | `.wf/README.md` |
