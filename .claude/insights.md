以下是我对这个项目的深度分析：

---

## Insight 1: 最大的设计张力 — 共享 Workflow

所有任务被强制走同一条流水线。这在标准化场景（同一个 Next.js 项目的多个 feature）下完美运作，但当任务类型不同时就尴尬了：

- 文档任务不需要 `bun install` / `typecheck`
- 纯重构任务不需要 code review
- hotfix 可能需要跳过 merge 直接 push

目前的规避方式是 `wf skip`，但这是手动操作。你可能需要考虑一种轻量的 per-task override 机制，比如在 task markdown frontmatter 里加 `skip_steps: [4, 5]` 或 `workflow: minimal`。

---

## Insight 2: `_on-exit` 残留风险

in_window 步骤使用 `trap EXIT` 触发 `wf _on-exit`，覆盖了正常退出和 SIGHUP（kill-window）。

~~**已解决** — `wf done` 和 `_on-exit` 竞争：Event Sourcing 迁移后，`replay()` 中 `OnExit` 事件仅当 `step_status` 中无该 step 记录时才生效，彻底防止了 double-advance。~~

残留风险：
1. **agent 进程 OOM/SIGKILL** → shell 被强杀 → trap 不执行 → 任务卡在 Running
2. **tmux server crash** → 所有窗口丢失 → 同上

解法建议：轻量 watchdog — 定期检查 `tmux::window_exists()`，窗口消失且状态还是 Running 则自动标记 Failed。

---

## Insight 3: ~~日志系统缺乏可靠性保证~~ [已解决]

Event Sourcing 迁移后，JSONL 是唯一的 source of truth，可靠性已大幅提升：

- **文件锁** — `append_event()` 使用 `fs2::lock_exclusive()` 保护并发写入
- **错误传播** — 所有 `append_event()` 调用使用 `?`，写入失败即任务失败，不再静默忽略
- **无 status.json** — 状态通过 `replay()` 从事件重建，消除了双源不一致的可能

---

## Insight 4: 你真正构建的是什么

表面上 wf 是一个 "workflow runner"，但它的独特价值不在于跑 shell 命令（make/just/task 都能做），而在于：

**它是一个 AI agent 的生命周期管理器。**

具体来说，它解决了三个其他工具不解决的问题：
1. **隔离**：每个 agent 在独立 worktree 中工作，互不干扰
2. **协议**：`done/fail/block` 提供了 agent 与编排器之间的通信协议
3. **可观测性**：TUI + capture + log 让人类能监控多个 agent 的实时状态

如果你要发布到 GitHub，我建议在 README 中强调这个定位，而不是 "workflow runner" —— 后者竞品太多，前者几乎是蓝海。

---

## Insight 5: Hook 系统有但没人用

Hook 系统实现完整（7 种事件，fire-and-forget，变量展开），但：
- 默认配置全被注释掉了
- `wf init` 没有引导用户配置 hooks
- 文档里没有重点介绍

这是一个杠杆点。想象这些场景：
```jsonc
{
  "hooks": {
    "task.completed": "say '${task} 完成了'",
    "step.failed": "osascript -e 'display notification \"${task}: ${step} failed\"'",
    "task.completed": "curl -X POST $SLACK_WEBHOOK -d '{\"text\": \"${task} completed\"}'"
  }
}
```

这些都不需要写代码，只需要在 init 或文档中引导一下。

---

## Insight 6: 状态机中的隐藏状态

当前 `TaskStatus` 有 6 种：`Pending / Running / Waiting / Completed / Failed / Stopped`

但实际上 `Running` 承载了两种完全不同的语义：
- 普通 step 正在同步执行（wf 进程自己在跑）
- in_window step 等待 agent 响应（wf 进程已退出，只是状态标记）

这导致 `wf status` 显示 "Running" 时，用户不知道是 "正在执行命令" 还是 "在等 AI agent 干活"。考虑加一个 `AgentWorking` 状态，或者在 status 输出中显示当前 step 的类型。

> 注：Event Sourcing 后，事件日志中可以通过 `WindowLaunched` 事件区分这两种状态，但 `replay()` 产出的 `TaskStatus` 仍然合并为 `Running`。

---

## Insight 7: 最大的未兑现承诺 — 并行

项目定位是"让多个 AI agent 并行开发"，但当前架构其实是：**人类手动并行启动多个串行流水线**。

```bash
wf start task-a  # 手动
wf start task-b  # 手动
wf start task-c  # 手动
```

真正的并行编排器应该能：
- `wf start --all` 启动所有就绪的任务
- 自动拓扑排序依赖（`depends` 字段已经有了）
- 依赖完成后自动启动下游任务

你的 `TaskDefinition.depends` 字段已经为此做好了数据准备，但执行引擎还没利用它。这可能是最有影响力的下一步功能。

---

## 总结：优先级建议

| 优先级 | 改进 | 影响 |
|--------|------|------|
| **P0** | 窗口 watchdog（检测 SIGKILL/tmux crash 僵尸状态） | 修复残留的卡死风险 |
| **P1** | `Running` vs `AgentWorking` 状态区分 | 改善可观测性 |
| **P1** | `wf start --all` + 依赖自动调度 | 兑现"并行开发"的核心承诺 |
| **P2** | Per-task workflow override | 扩展使用场景 |
| **P2** | Hook 默认启用（macOS notification） | 低成本用户体验提升 |
| ~~P3~~ | ~~日志写入可靠性~~ | ~~已通过 Event Sourcing 解决~~ |