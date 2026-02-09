# Session Handoff

## Current Session (S45): Driver Adapter + 监工体验 + Viewport 精简

### What changed

**1. Driver Adapter：4 操作 → 2 操作（start + read）**

`claude-driver.sh` 从 start/send/stop/read 简化为 start + read。send/stop 是 substrate（tmux send-keys / pawl stop），不属于 adapter。

- `start`：通过 `[ -t 0 ]` 自动检测 pipe vs TUI 模式
- `read`：推导 Claude Code session log 路径，输出 JSONL
- 默认参数 `${1:-start}`（无参数时默认 start）

**2. orchestrate.md 精简**

- Agent Driver 章节：2 操作模板 + pipe/TUI 双模式 config 示例 + 完成检测说明
- 删除整个 `## Claude Code CLI for Workflows` 章节（CC flags 表、CC Adapter 实例化、Plan-Then-Execute、Structured Output）——编排者不需要 CC 专属知识，已封装在 adapter 里
- 省 token 参数挪到 `claude-driver.sh` 头部注释

**3. 修复 `done -m` 消息丢失 bug**

`pawl done -m "reason"` 在 Waiting 路径（gate/manual approval）下 message 被静默丢弃。违反 `state = replay(log)`。

修复：`StepResumed` 事件加 `message: Option<String>` 字段 + hook `${message}` 变量。

**4. 监工体验实测**（详见下方独立章节）

**5. 删除 `pawl capture` + `pawl enter`，精简 viewport trait**

通过监工实测 + essence 分析确认：
- capture 引导监工看 terminal buffer（stale、非结构化），偏离 `state = replay(log)` 真相源
- enter 是 substrate 封装（`tmux select-window`），唯一消费者是人类，pawl 是 agent-first
- `{需要 in_viewport} ∩ {简单脚本} ∩ {需要详细日志}` = ∅，capture 无独占用例

Viewport trait 从 7 → 4 方法：`open, execute, exists, close`。删除 `read, is_active, attach`。

### 监工体验记录

以 pawl 自身作为实验对象，完整走了两轮监工流程。

**第一轮（同步 + gate）**：prepare → work(verify+retry) → review(gate) → deliver。体验了 status → log → done 的基本循环。发现 done -m 消息丢失 bug。

**第二轮（异步 in_viewport + verify 失败 + retry）**：

搭建了一个有 bug 的 Rust 项目（fibonacci off-by-one），用模拟 agent 脚本修代码，`cargo test` 做 verify。

实际流程：
1. `pawl start` 立刻返回（in_viewport 异步）
2. Agent 第一次只修 base case → verify 失败（`left:3 right:5`）→ auto retry
3. Agent 第二次修 loop bound → verify 通过 → gate 等待
4. 监工检查 `log --all`（verify_output 有完整测试输出）、`capture`（stale/误导）、直接 `cargo test`
5. `pawl done -m "Both fixes verified..."` 审批 → commit 自动执行

**关键发现**：

| 发现 | 分类 | 结论 |
|------|------|------|
| verify 的 `cmd \| tail -5` 吞退出码 | 编排者错误 | trust the substrate，不是 pawl 的问题 |
| capture 显示 stale 输出，有误导性 | 设计矛盾 | 与 `state = replay(log)` 冲突 → 删掉 |
| `log --all` 的 verify_output 比 capture 可靠 | 验证设计 | 日志是监工最重要的工具 |
| status 的 prompt 字段直接路由决策 | 验证设计 | agent 不需要理解 pawl 内部 |
| done -m 消息在 gate 路径丢失 | 真 bug | 已修复（StepResumed.message） |

**设计洞察**：pawl 的事件日志是监工的真相源。capture 是错误的旁路——它鼓励监工看 terminal buffer 而非日志。删掉后监工被引导到 `log`（审计轨迹）和 driver `read`（agent 日志），这两个都更可靠。

### S44 遗留问题处置

| S44 遗留 | 处置 |
|---------|------|
| Viewport CRUD 不对称（缺 write/send） | 已解决：send 是 substrate（tmux send-keys），不属于 viewport trait |
| `pawl stop` 是否关 viewport | 未检查，仍 pending |
| Driver 4 操作 vs essence | 已解决：adapter = start + read，send/stop = substrate |
| Settings.json Stop hook 集成 | 仍 pending |
| TUI mode prompt injection 时序 | 仍 pending（`[ -t 0 ]` 检测已实现，但 TUI 模式下的 prompt 注入方案未定） |

---

## Previous Sessions (compressed)

### S44: Agent Driver 概念 + orchestrate.md 重构
- 从 foreman-worker 模型推导 driver 4 操作（start/send/stop/read）
- orchestrate.md recipe 重排：Plain Workflow 排第一，解耦 worktree 依赖
- 实测验证（sonnet agent + stdin + retry resume）

### S43 and earlier
- S43: Agent 本质讨论 + 验证，hook insight
- S42: Skill doc stress test + viewport close 时序 fix
- S41: 9 agent E2E tests
- S39: 104 E2E tests (72 sync + 32 viewport)
- S38: Decouple git, add config.vars
- S36-S37: Less-Is-More Audit + Skill self-containment
- S32-S35: Agent-First Interface + Role-based skill architecture

---

## Pending Work

1. **确认 `pawl stop` 行为**：检查 control.rs，是否需要加 viewport.close()
2. **Settings.json Stop hook 集成**：claude-driver.sh 引用 co-located settings.json
3. **TUI mode prompt injection**：`[ -t 0 ]` 已实现，但 prompt 注入到 TUI 的时序方案未定

## Known Issues

None (kernel level).

## Key File Index

| Area | File |
|------|------|
| CLI definition (12 commands) | `src/cli.rs` |
| PawlError enum (6 variants, exit codes 2-7) | `src/error.rs` |
| Project context, context_for, output_task_state | `src/cmd/common.rs` |
| Status + derive_routing (suggest/prompt) | `src/cmd/status.rs` |
| Execution engine, settle_step, decide() | `src/cmd/start.rs` |
| in_viewport parent process (`pawl _run`) | `src/cmd/run.rs` |
| Done/approve handler (StepResumed.message) | `src/cmd/done.rs` |
| Stop/Reset handler | `src/cmd/control.rs` |
| Wait (poll with Timeout) | `src/cmd/wait.rs` |
| Event model + replay (10 types) | `src/model/event.rs` |
| Viewport trait (open/execute/exists/close) | `src/viewport/mod.rs` |
| Templates (config + skill + references + driver) | `src/cmd/templates/` |
| E2E tests (sync, 72 tests) | `tests/e2e.sh` |
| E2E tests (viewport, 27 tests) | `tests/e2e-viewport.sh` |
| E2E tests (agent, 9 tests) | `tests/e2e-agent.sh` |
