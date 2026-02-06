# Session Handoff

## 本次 Session 完成的工作

### E2E 端到端测试（全 13 命令）

在 mock 项目中对 wf CLI 全部 13 个命令 + 1 个内部命令进行了完整端到端测试。

**测试方法**: 用 8 步轻量工作流替代生产配置，覆盖所有步骤类型（normal, verify:command, on_fail:retry, gate, verify:human, in_window, on_fail:human）。

**测试结果**: 全部 PASS（除发现 1 个 bug 并当场修复）。

覆盖的场景：
- 基础命令: init(重复报错), create(正常/依赖/重复), list, status(文本/JSON)
- Happy Path: setup→build→flaky-test(重试2次)→gate→review(verify:human)→develop(in_window)→risky-deploy(on_fail:human)→cleanup
- Skip: 4 个步骤跳过（step_skipped 事件）
- 依赖: 阻塞/满足后启动
- 生命周期: stop(Running→Stopped), reset(→Pending), reset --step(步骤重试)
- wait: 已达到状态立即返回, 等待 waiting, 超时报错
- _on-exit: exit_code=0(继续), exit_code=1(Failed)
- Window Lost: kill -9 杀 shell → wf status 健康检查自动检测
- 错误条件: 7 种非法操作全部正确报错
- 事件钩子: on.task_started/step_completed 变量展开正确
- log: 最新事件, --step N 过滤, --all 全部
- capture: 文本/JSON 格式, enter 切换窗口

### Bug 修复: StepWaiting 不更新 current_step

- **文件**: `src/model/event.rs`
- **问题**: `StepCompleted(exit_code=0)` 将 `current_step` 推进到 `step+1`，随后 `StepWaiting` 不修改 `current_step`。`wf done` 读取错误的 `current_step` 批准了下一个步骤，导致步骤被跳过。
- **影响**: verify:human 等待后 `wf done` 会跳过下一步（如 develop in_window 被完全跳过）
- **修复**: `StepWaiting` handler 中增加 `s.current_step = *step`
- **回归测试**: `test_step_waiting_after_completed_resets_current_step`
- 28 测试全部通过

---

## 历史 Session

### Session 3: Unified Step Pipeline 重构
- Event 14→11, CLI 19→13, ~570 行净删除
- `handle_step_completion()` + `apply_on_fail()` 统一管线
- TaskDefinition 新增 skip, Greenfield 清理 dead code

### Session 2: Step 验证模型改造
- Step 迁移到 4 正交属性（run, verify, on_fail, in_window）
- 事件重命名, 新增 VerifyFailed, verify helpers

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
