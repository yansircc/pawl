# Session Handoff

## 本次 Session 完成的工作

### E2E 确认的 3 个真问题修复 + 3 个新痛点修复

通过 E2E Foreman 视角测试（9 场景 × 3 组并行），修复了 6 个问题：

**第一批（计划内）**：

| # | 问题 | 修复 | 文件 |
|---|------|------|------|
| P12 | `tmux kill-window` 导致 exit_code=0 误判成功 | `on_exit()` 中检查窗口存在性，不存在则发 WindowLost | `control.rs` |
| P4 | StepWaiting 缺 reason 字段，三种 Waiting 不可区分 | 加 `reason: String` 字段（"gate"/"verify_human"/"on_fail_human"） | `event.rs`, `start.rs`, `log.rs` |
| P3 | `wf wait` 不做 window health check | poll 循环中检查 in_window Running 窗口是否存在 | `wait.rs` |

**第二批（E2E 发现的新痛点）**：

| # | 问题 | 修复 | 文件 |
|---|------|------|------|
| P9 | run 失败时 `status --json` 的 `last_feedback` 为空 | `extract_step_context()` 也从 StepCompleted(exit!=0) 提取 stdout/stderr | `status.rs` |
| - | `wf done` 对未启动 task 报 "not found" 而非 "not started" | 改 bail 消息 | `approve.rs`, `control.rs` |
| - | verify/on_fail 无 run 配置时无提示 | Config 加载后 eprintln warning；修复 init 模板 | `config.rs`, `init.rs` |

**P12 回归验证**：快速退出的 in_window 命令（echo 秒退）不会被误判为 window_lost——on_exit trap 在窗口关闭前执行。

**E2E 9 场景全 PASS**：Foreman 闭环、并发 task、start --reset、retry 自动重试、verify 失败 feedback、步骤失败 stdout 可达性、events --follow、快速 in_window、done 非法状态。

代码净变化：+117 -25 行，零 warning，29/29 测试通过。

---

## 历史 Session

### Session 10: E2E Foreman 视角测试
- 8步 workflow × 3 task × 12 phase 全自动测试
- 发现 P12/P4/P3/P1/P5/P9 等痛点（本次 session 修复了 P12/P4/P3/P9）

### Session 9: 辩论驱动改进
- Step 编号统一 0-based、`wf start --reset`、`wf events --follow`、`wf log` 当前轮

### Session 8: P1-P5 改进 + Greenfield 清理
- `wf log --jsonl`、`wf wait --until` 多状态、`wf status --json` 增强、`done()` 统一管线

### Session 5-7: Foreman 模式 + AI Worker 闭环
- 非交互 Claude 集成、wrapper.sh 模式、事件 hook 通知、并发 task

### Session 1-4: 架构演进
- TUI 删除 → Event Sourcing → Step 模型 → Unified Pipeline → E2E 测试

---

## 已知监控项

- `extract_step_context()` (status.rs) 与 `count_auto_retries()` (start.rs) 逻辑更相似了（都扫 StepCompleted），但两处用途不同（提取 feedback vs 计数），暂保留
- `wf events` 输出全部历史事件（不按当前轮过滤），与 `wf log --all` 行为不一致
- Session 10 中 P1(stdout 无大小限制) 和 P5(verify 失败 feedback 为空) 未修——P5 实测发现 feedback 有内容（误报），P1 暂无实际影响

## 关键文件索引

| 功能 | 文件 |
|------|------|
| CLI 定义（14 命令） | `src/cli.rs` |
| 配置模型（Step 4 属性 + 校验） | `src/model/config.rs` |
| 事件模型 + replay（11 种，StepWaiting 含 reason） | `src/model/event.rs` |
| 状态投影类型 | `src/model/state.rs` |
| 任务定义（含 skip） | `src/model/task.rs` |
| 执行引擎 + 统一管线 | `src/cmd/start.rs` |
| 审批命令（done，统一管线） | `src/cmd/approve.rs` |
| 控制命令（stop/reset/on_exit + P12 窗口检查） | `src/cmd/control.rs` |
| 状态输出（retry_count/last_feedback 含 run 失败） | `src/cmd/status.rs` |
| 日志输出（当前轮/全历史/--jsonl） | `src/cmd/log.rs` |
| 统一事件流（--follow 实时监听） | `src/cmd/events.rs` |
| 等待命令（多状态 + window health check） | `src/cmd/wait.rs` |
| 初始化（含 lib 模板生成） | `src/cmd/init.rs` |
| 公共工具（事件读写、钩子） | `src/cmd/common.rs` |
| 变量上下文 | `src/util/variable.rs` |
| 项目概述 | `.claude/CLAUDE.md` |
