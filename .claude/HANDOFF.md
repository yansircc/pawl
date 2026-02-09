# Session Handoff

## Current Session (S46): 遗留项清理 + 端到端监工测试 + 文档修复

### What changed

**1. 关闭 S44 以来的 3 个 pending 项（全部"不改"）**

| 遗留项 | 结论 | 核心推理 |
|--------|------|---------|
| `pawl stop` 加 viewport.close() | 不加 | stop 是状态操作，不是资源生命周期。done 的 close 是 resume 时序约束的解法，stop 无 resume |
| Settings.json Stop hook | 不需要 | _run 已完整捕获 child exit → settle_step |
| TUI mode prompt injection | 已解决 | 初始 prompt: driver 传参/stdin。retry: `-r $PAWL_RUN_ID`。运行中: substrate |

推理教训：S44 agent 通过对称性审计发现疑似遗漏，标记 pending。接手时容易掉入**类比推理**（"done 做了所以 stop 也该做"），跳过因果分析。用 /rethink 打破了锚定。

**2. 端到端监工测试（Go CLI 项目 `jot`）**

在 /tmp/pawl-tui-test/ 搭建了完整的 pawl 监工流程测试：
- 7 个 task（scaffold → storage → cmd-add/list/search/tags-delete/export）
- fake agent 脚本（agent.sh，根据 $PAWL_TASK 生成 Go 代码）
- TUI 模式验证通过
- cmd-search 触发了真实 retry（agent.sh 有 bug：test 文件缺 import "strings"）
- 完整经历：auto-retry 3 次 → 耗尽 → failed → 手动修 agent.sh → reset --step → 成功

6 个摩擦点分析结果：0 个需要改 pawl 代码。2 个是编排者错误（过度使用 gate），4 个符合设计哲学。

**3. 文档修复（3 处）**

| 文件 | 改动 |
|------|------|
| `supervise.md` | 新增 **Log (inspection)** 章节：`pawl log` 三种用法 + verify_output 说明 |
| `orchestrate.md` | Step Properties Rules 后追加 **gate 决策指导** |
| `author.md` | `depends` 字段标注 **enforced**（`pawl start` 拒绝 + exit 3） |

实测确认 `depends` 是强制执行的（之前误以为是 informational）。

### 决策记录

- **拒绝 batch start / auto-advance**：trust the substrate，shell 循环就是批量机制
- **拒绝 `pawl list` 人类友好格式**：agent-first 设计，jq 是 formatting substrate
- **拒绝 retry_count 累计计数**：auto retry 和 manual reset 语义不同，各有正确行为
- **确认 depends 是 enforced**：`pawl start` 检查依赖，未满足返回 exit 3 (Precondition)

---

## Previous Sessions (compressed)

### S45: Driver Adapter + 监工体验 + Viewport 精简
- Driver adapter 简化为 2 操作（start + read），send/stop 归 substrate
- 修复 `done -m` 消息丢失 bug（StepResumed.message）
- 监工实测两轮（同步+gate / 异步+verify+retry），确认日志是监工真相源
- 删除 capture/enter，viewport trait 从 7→4 方法

### S44: Agent Driver 概念 + orchestrate.md 重构
- 从 foreman-worker 模型推导 driver 操作
- orchestrate.md recipe 重排：Plain Workflow 排第一，解耦 worktree 依赖

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

None.

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
