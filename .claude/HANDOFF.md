# Session Handoff

## 本次 Session 完成的工作

### Session 13: P0/P1/P2 重构 + Greenfield 清理

**代码改动**（36 tests 全过，零 warning）：

| 改动 | 文件 | 说明 |
|------|------|------|
| **P0: resolve/dispatch** | `start.rs` | Action 枚举 + resolve() 纯函数(7 单元测试) + dispatch() IO 函数。删除 apply_on_fail，重写 handle_step_completion 为三步组合 |
| **P1: WindowLost 统一** | `common.rs` | 新增 check_window_health() 作为唯一 WindowLost 发射点，删除 replay_task_with_health_check() |
| **P1: 调用点替换** | `status.rs` `capture.rs` `control.rs` | 4+1+1 处 replay_task_with_health_check → check_window_health + replay_task |
| **P2: wait.rs API** | `wait.rs` | 完全走 Project API，删除 replay_state_from_file/append_window_lost，简化 poll_status 签名 |
| **Greenfield** | `event.rs` | 新增 pub count_auto_retries() 共享函数 |
| **Greenfield** | `status.rs` | extract_step_context 使用 event::count_auto_retries |
| **Greenfield** | `config.rs` | 删除未使用的 on_fail_retry/on_fail_human |

**解决的审计项**：V1(查询混写副作用) V2(event hook 失效) V3(WindowLost 竞态) V7(双权威竞态缓解) V8(wait 第三裁决者)

---

## 历史 Session

### Session 12: 第一性原理审视 + VerifyFailed 消除
- Phase 1: 事件 11→10，StepCompleted 发射收归 handle_step_completion
- 三 agent 审计发现 11 项规则违反，P0/P1/P2 重构计划

### Session 9-11: 辩论驱动改进 + E2E 修复
- Step 0-based 统一、start --reset、events --follow、log 当前轮
- P12 窗口检测、P4 Waiting reason、P3 wait health check

### Session 5-8: Foreman 模式 + P1-P5
- 非交互 Claude、wrapper.sh、事件 hook、并发 task

### Session 1-4: 架构演进
- TUI 删除 → Event Sourcing → Step 模型 → Unified Pipeline → E2E 测试

---

## 已知监控项

- **on_exit + wf done 双权威竞态**: in_window 步骤两个裁决者可同时触发 (V7 缓解但未完全消除)
- **on_exit 丢失 RunOutput**: in_window 进程退出无 stdout/stderr/duration
- **retry 耗尽无审计事件**: 从 retry 转终态时无事件记录 (V10)
- **verify:human 崩溃瞬态**: 两个 append 间崩溃窗口极小 (V5)
- `wf events` 输出全部历史（不按当前轮过滤），与 `wf log --all` 不一致

## 关键文件索引

| 功能 | 文件 |
|------|------|
| CLI 定义（14 命令） | `src/cli.rs` |
| 配置模型（Step 4 属性） | `src/model/config.rs` |
| 事件模型 + replay + count_auto_retries（10 种） | `src/model/event.rs` |
| 状态投影类型 | `src/model/state.rs` |
| 任务定义（含 skip） | `src/model/task.rs` |
| 执行引擎 + resolve/dispatch 管线 | `src/cmd/start.rs` |
| 审批命令（done） | `src/cmd/approve.rs` |
| 控制命令（stop/reset/on_exit + check_window_health） | `src/cmd/control.rs` |
| 状态输出（retry_count/last_feedback） | `src/cmd/status.rs` |
| 日志输出（当前轮/全历史/--jsonl） | `src/cmd/log.rs` |
| 统一事件流（--follow） | `src/cmd/events.rs` |
| 等待命令（Project API + check_window_health） | `src/cmd/wait.rs` |
| 初始化（含 lib 模板） | `src/cmd/init.rs` |
| 公共工具（事件读写、钩子、check_window_health） | `src/cmd/common.rs` |
| 变量上下文 | `src/util/variable.rs` |
| 重构设计文档 | `.claude/specs/event-model-refactor.md` |
| 项目概述 | `.claude/CLAUDE.md` |
