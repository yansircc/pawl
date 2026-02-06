# Core Concepts

## 概念层级

```
Project
  ├── Config        # workflow 定义 + hooks
  └── Task[]        # 任务列表，各自独立推进
        ├── name        # 任务名
        ├── depends     # 依赖的任务
        ├── description # 任务描述 (markdown)
        └── state       # 运行时状态 (current_step + status)
```

## Step

最小执行单元。一条 shell 命令。

```jsonc
{ "name": "Type check", "run": "bun typecheck" }
```

Step 有两种执行方式：

| 属性 | 行为 | 完成条件 |
|------|------|---------|
| 普通 step | 同步执行，等待退出 | exit code |
| `in_window: true` | 发送到 tmux window 执行 | `wf done` / `wf fail` / 进程退出 |

### Step 状态

```
pending → running → success
                  → failed
                  → blocked
                  → skipped (被 wf skip 跳过)
```

### 状态判定

| 来源 | 结果 |
|------|------|
| exit 0 | success |
| exit 非 0 | failed |
| `wf done` | success (覆盖 exit code) |
| `wf fail [reason]` | failed |
| `wf block [reason]` | blocked |

对于 `in_window` 的 step，`wf done/fail/block` 优先于 exit code。如果进程退出时没有主动标记状态，由 `wf _on-exit` 兜底处理。

## Checkpoint

没有 `run` 的 step。暂停 workflow，等待人工 `wf next`。

```jsonc
{ "name": "确认开发完成" }
```

Checkpoint 是人为制造的分割点，让人在关键节点审查和决策。

未来（post-MVP）：checkpoint 处可以自动做 git 快照，支持 `wf back` 回退。

## Workflow

Step 的有序列表。所有任务共享同一个 workflow 定义。

```jsonc
"workflow": [
  { "name": "Create branch", "run": "git branch wf/${task}" },
  { "name": "Develop", "run": "claude ...", "in_window": true },
  { "name": "确认开发完成" },
  { "name": "Type check", "run": "bun typecheck" },
  { "name": "Cleanup", "run": "..." }
]
```

执行规则：
1. 从 `current_step` 开始，顺序执行
2. 普通 step：同步等待完成，成功则 `current_step++`
3. `in_window` step：发送到 window 后暂停，等待状态标记
4. checkpoint：暂停，等待 `wf next`
5. 任何 step 失败：暂停，等待人工介入
6. 全部执行完：任务标记 completed

## Event Hook

事件触发的 shell 命令，在 `append_event()` 写入 JSONL 后自动触发。

```jsonc
"on": {
  "task_started": "echo '${task} started'",
  "command_executed": "echo '${task} step ${step} exit=${exit_code}'",
  "agent_reported": "echo '${task} agent: ${result} ${message}'",
  "window_lost": "echo '${task} window crashed'"
}
```

key 为 Event enum 的 serde tag（snake_case），所有 13 种事件类型均可挂载 hook。事件特有字段（`${exit_code}`、`${result}`、`${message}`、`${session_id}`、`${duration}`）自动注入。

Hook 的执行是 fire-and-forget，不影响 workflow 推进。Hook 失败只打印警告，不阻塞。

## Task

一个独立的工作单元。定义为 markdown 文件。

```yaml
# .wf/tasks/auth.md
---
name: auth
depends:
  - database
---
实现用户认证...
```

### Task 状态

```
pending → running → waiting → running → ... → completed
                  → failed
```

| 状态 | 含义 |
|------|------|
| `pending` | 尚未开始（或依赖未满足）|
| `running` | 某个 step 正在执行 |
| `waiting` | 等待条件（checkpoint / in_window / 人工介入）|
| `completed` | workflow 全部完成 |
| `failed` | 某个 step 失败，等待人工介入 |

### 依赖

Task 可以声明依赖：

```yaml
depends:
  - task-a
  - task-b
```

规则：所有依赖的 task 必须 `completed` 后，当前 task 才能 `wf start`。

## 变量

> **v1 参考**：变量展开逻辑 `/Users/yansir/code/tmp/worktree/src/services/executor/context.rs:114-173`
> 环境变量设置 `/Users/yansir/code/tmp/worktree/src/services/executor/context.rs:175-213`

Step 的 `run` 命令支持变量展开：

| 变量 | 值 | 来源 |
|------|------|------|
| `${task}` | 任务名 | task.name |
| `${branch}` | `wf/{task}` | 派生 |
| `${worktree}` | `.wf/worktrees/{task}` (绝对路径) | 派生 |
| `${window}` | `{task}` | 派生 |
| `${session}` | config.session | 配置 |
| `${repo_root}` | git repo 根目录 (绝对路径) | 运行时检测 |
| `${step}` | 当前 step name | 运行时 |

所有变量都可从 task name + config 确定性派生，不需要存储。

## 设计原则

1. **一切皆 shell 命令** — Step、Hook、验证，都是 shell 命令
2. **约定优于配置** — 变量从 task name 派生，不需要手动指定
3. **人在回路** — Checkpoint 让人在关键节点介入
4. **可观测** — 每个 step 有 log，状态变化可追踪
5. **最小状态** — 只存 current_step + status，其他全部派生
