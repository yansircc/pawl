# Session Handoff

## 本次 Session 完成的工作

### Session 14: E2E 包工头测试 + 痛点修复 + Foreman Guide

**E2E 测试**（8 步工作流 × 3 task × 16 场景）：
- A 组: Happy path（全流程、skip、start --reset）
- B 组: 失败与恢复（verify retry、retry 耗尽、on_fail:human、reset --step）
- C 组: in_window + tmux（正常完成、window_lost、wf enter、wf capture）
- D 组: 并发与边界（多 task 并发、wf wait 多状态、wf stop、边界条件）
- E 组: CLI 输出格式（status --json、log、events、list）
- F 组: Event Hook（触发验证、变量展开）

**痛点修复**（6 个文件，36 tests 全过）：

| 修复 | 文件 | 说明 |
|------|------|------|
| `wf stop` 支持 Waiting 状态 | `control.rs` | 接受 Running\|Waiting，符合规则 5 "failure is routable" |
| `wf list` 显示等待原因 | `status.rs` | INFO 列: gate / needs review / needs decision |
| Completed 显示 Step 8/8 | `status.rs` | 不再泄露内部 9/8，detail 和 list 视图统一修复 |
| `wf reset` 条件输出 git 提示 | `control.rs` | 检测 worktree/branch 存在才提示清理 |
| `wf start` Waiting 错误加上下文 | `start.rs` | 报 "waiting at step N (name) for reason" |
| in_window 窗口不抢焦点 | `tmux.rs` | `new-window -d` 后台创建 |
| `branch_exists()` 辅助函数 | `git.rs` | 支持 reset 条件检测 |

**Foreman Guide + Config 模板**：

| 产物 | 位置 | 说明 |
|------|------|------|
| 包工头操作手册 (222 行) | `init.rs` → `.wf/lib/foreman-guide.md` | 心智模型、主循环、5 种决策场景、注意事项 |
| Config 详细注释 (145 行) | `init.rs` → `.wf/config.jsonc` | Step 4 属性、6 种类型速查、变量表、设计建议、Hook 参考 |

两份文件在 `wf init` 时自动生成。

---

## 历史 Session

### Session 13: P0/P1/P2 重构 + Greenfield
- resolve/dispatch 分离（7 单元测试）、WindowLost 统一、wait.rs 走 Project API

### Session 12: 第一性原理审视 + VerifyFailed 消除
- 事件 11→10，StepCompleted 统一发射，三 agent 审计

### Session 9-11: 辩论驱动改进 + E2E
- Step 0-based 统一、start --reset、events --follow、log 当前轮

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
| 控制命令（stop/reset/on_exit） | `src/cmd/control.rs` |
| 状态输出（retry_count/last_feedback/waiting reason） | `src/cmd/status.rs` |
| 日志输出（当前轮/全历史/--jsonl） | `src/cmd/log.rs` |
| 统一事件流（--follow） | `src/cmd/events.rs` |
| 等待命令（Project API + check_window_health） | `src/cmd/wait.rs` |
| 初始化（含 config 模板 + foreman guide + lib） | `src/cmd/init.rs` |
| 公共工具（事件读写、钩子、check_window_health） | `src/cmd/common.rs` |
| tmux 工具（窗口后台创建） | `src/util/tmux.rs` |
| git 工具（branch_exists） | `src/util/git.rs` |
| 变量上下文 | `src/util/variable.rs` |
| 包工头操作手册 | `.wf/lib/foreman-guide.md` |
| 项目概述 | `.claude/CLAUDE.md` |
