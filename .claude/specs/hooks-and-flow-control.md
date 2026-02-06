# wf 架构设计

## 核心哲学

- **事件是已经发生的事实。你不拦截事实，你只响应事实。**
- **Agent 是函数，不是进程。** 接受输入，产生输出。不需要感知 wf 的存在。
- **人类是验证器的一种。** 不是特殊的流程节点。

## Step 模型

Step 有四个正交属性：

| 属性 | 回答的问题 | 可选值 |
|------|-----------|--------|
| `run` | 做什么 | shell 命令（可选，无则为纯验证门控） |
| `verify` | 谁来判断做得对不对 | 命令 / AI agent / `"human"`（可选） |
| `on_fail` | 做错了怎么办 | `"retry"` / `"human"` / 默认 Failed |
| `in_window` | 人要不要看着 | bool（可观测性 + 可交互性） |

### on_fail 语义

| 值 | 行为 |
|----|------|
| `"retry"` | 自动重试，把 verify feedback 注入下次执行。配合 `max_retries`（默认 3） |
| `"human"` | 暂停（Waiting），展示 feedback，等人决定 |
| 默认（不填） | 标记 Failed，等手动 `wf retry` / `wf skip` |

### 合法组合

| verify | on_fail | 含义 |
|--------|---------|------|
| 命令 | `"retry"` | 全自动：验证失败 → 注入 feedback → 重跑 |
| 命令 | `"human"` | 半自动：验证失败 → 暂停等人决定 |
| 命令 | 默认 | 标记 Failed，等手动 retry |
| `"human"` | `"retry"` | 人类判生死，系统自动重跑 agent |
| `"human"` | 默认 | 人类判且人类操作 |

`verify: "human"` + `on_fail: "human"` 无意义，等于默认。

### 执行流程

```
Step 开始
  ├─ 有 run？
  │   ├─ in_window: false → 同步执行命令，等待完成
  │   └─ in_window: true  → 送入 tmux，等待进程退出（_on_exit 捕获）
  │
  └─ run 完成后（或无 run）
      ├─ 有 verify？
      │   ├─ "human"  → status=Waiting，等待 wf done/fail
      │   └─ 命令     → 执行验证命令
      │       ├─ exit 0  → 验证通过，推进
      │       └─ exit !0 → 验证失败
      │           ├─ on_fail="retry" → 注入 feedback，重跑 step（max_retries 限制）
      │           ├─ on_fail="human" → status=Waiting，展示 feedback
      │           └─ 默认 → status=Failed
      │
      └─ 无 verify → 根据 run 的 exit code 判断成败
```

### 示例

```jsonc
// 普通命令
{ "name": "setup", "run": "cd ${worktree} && npm install" }

// 人工验证门控（取代旧 checkpoint）
{ "name": "confirm-ready", "verify": "human" }

// 命令 + 人工验证
{ "name": "setup", "run": "setup.sh", "verify": "human" }

// Agent + 自动验证 + 自动重试
{
  "name": "develop",
  "run": "claude -p '${task_file}'",
  "in_window": true,
  "verify": "cd ${worktree} && npm run typecheck && npm run lint",
  "on_fail": "retry",
  "max_retries": 3
}

// Agent + AI 验证器 + 失败后人工介入
{
  "name": "develop",
  "run": "claude -p '${task_file}'",
  "in_window": true,
  "verify": "cd ${worktree} && claude -p 'Review the code. Exit 0 if good, exit 1 with feedback.'",
  "on_fail": "human"
}

// Agent + 人类验证 + 自动重跑
{
  "name": "develop",
  "run": "claude -p '${task_file}'",
  "in_window": true,
  "verify": "human",
  "on_fail": "retry"
}
```

## Verify 反馈传递

### 自动验证（_on_exit 驱动，agent 不感知 wf）
```
Agent 退出 → _on_exit → verify → fail → feedback 存入事件
  → on_fail="retry" → 重启 agent，feedback pipe 进输入
  → on_fail="human" → 暂停，人看 feedback 后决定
  → 默认 → Failed，等手动操作
```

### 人类验证
```
Step 完成 → status=Waiting → 人类审查
  → wf done  → 通过，推进
  → wf fail -m "理由" → feedback 存入事件
    → on_fail="retry" → 自动重跑 agent
    → 默认 → Failed
```

### Agent 主动上报（可选，向下兼容）
```
Agent 运行中 → wf done → verify → fail → agent 看到反馈 → 修复 → 再次 wf done
```

## 两层架构

| 层 | 机制 | 性质 | 时机 |
|----|------|------|------|
| Event 消费层 | `on` (hooks) | fire-and-forget, 不影响 workflow | 事件写入后 |
| Command 验证层 | `verify` | 同步阻塞, 可拒绝状态转换 | 事件产生前 |

## 概念消除

| 旧概念 | 新概念 | 说明 |
|--------|--------|------|
| checkpoint | `verify: "human"` | 人是验证器的一种 |
| stop_hook | `verify` | 重命名，语义更准确 |
| `block`（AgentResult） | 删除 | `on_fail: "human"` 覆盖了"需要人介入"的语义 |
| agent 必须调用 `wf done` | 可选 | `_on_exit` + verify 自动处理 |
| `wf block` 命令 | 删除 | 用 `wf fail -m "reason"` 替代 |
| 全局 hooks（手动 fire_hook） | `on`（自动挂载） | 单一触发点 |
