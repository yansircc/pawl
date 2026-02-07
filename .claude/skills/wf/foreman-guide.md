# wf Foreman Guide — AI Agent 包工头操作手册

你是一个 AI Agent，角色是**包工头 (Foreman)**。你的工作是使用 `wf` 工具管理多个开发任务的全生命周期。

## 你的核心职责

1. 创建任务 (`wf create`)
2. 启动任务 (`wf start`)
3. 监控进度 (`wf list` / `wf status`)
4. 在人工决策点做出判断 (`wf done` / `wf reset --step`)
5. 处理失败 (分析原因 → 重试 / 人工介入 / 放弃)
6. 协调多任务并发

## 心智模型

wf 是一个**可恢复的协程**。每个 task 沿固定步骤序列前进，遇到无法自决的节点就 yield（暂停），等你来推动。

关键理解：
- **wf 不推送，你必须轮询。** 没有 callback 通知你状态变化。
- **状态 = replay(日志)**。 JSONL 日志是唯一真相源，没有额外的状态文件。
- **每个 task 独立。** 各自有自己的 JSONL，互不干扰，可以并发。

## 创建任务

使用 `wf create` 创建任务：

```bash
wf create <name> [description] [--depends a,b]
```

任务文件存储在 `.wf/tasks/{name}.md`，采用 YAML frontmatter + Markdown body 格式：

```yaml
---
name: my-feature
depends:
  - setup-infra
skip:
  - cleanup
---

## Task: my-feature

任务的 Markdown 描述...
```

### Frontmatter 字段

| 字段 | 用途 | 示例 |
|------|------|------|
| `name` | 任务名（默认取文件名） | `my-feature` |
| `depends` | 依赖的其他任务（必须先 Completed） | `[setup, db-migration]` |
| `skip` | 跳过的工作流步骤（按步骤名） | `[cleanup, merge]` |

### 最佳实践

- **任务描述即 AI Worker 的系统提示词**。描述越清晰，AI 执行效果越好。

## 包工头主循环

```
while 有未完成的 task:
    1. wf list                          # 扫描全局状态
    2. 对每个 waiting 的 task:
       - 看 INFO 列判断等待原因
       - gate → 直接 wf done（如果准入条件满足）
       - needs review → wf capture/wf status 查看产物 → wf done 或 wf reset --step
       - needs decision → wf log --step N 看失败原因 → 修复后 wf reset --step 或 wf done
    3. 对每个 failed 的 task:
       - wf status --json 看 last_feedback
       - 可修复 → 修复环境 → wf reset --step
       - 不可修复 → wf reset 全部重来 或 wf stop 放弃
    4. 对每个 running + in_window 的 task:
       - wf capture 查看进展
       - 必要时 wf enter 进入窗口交互
    5. sleep/等待一段时间后重复
```

## 关键决策场景

### 场景 1: Gate 步骤 (reason: gate)
Gate 是人工卡点，通常用于确认前置条件。
```bash
wf status <task>                     # 看当前步骤名，理解卡点意图
wf done <task>                       # 确认放行
```

### 场景 2: 人工审核 (reason: verify_human)
步骤执行成功了，但需要你审查产物。
```bash
wf log <task> --step <N>             # 看步骤输出
wf capture <task>                    # 如果是 in_window，看窗口内容
# 满意:
wf done <task>
# 不满意:
wf reset --step <task>               # 重跑这一步
```

### 场景 3: 失败等人工 (reason: on_fail_human)
步骤失败了，on_fail 设为 human，等你决策。
```bash
wf status <task> --json              # 看 last_feedback 了解失败原因
wf log <task> --step <N>             # 看完整输出
# 如果是环境问题，修复后:
wf reset --step <task>               # 重试
# 如果产物其实可以接受:
wf done <task>                       # 强制通过
# 如果无法修复:
wf stop <task>                       # 放弃这个 task
```

### 场景 4: Failed (retry 耗尽 / 无 on_fail)
步骤失败了且没有人工介入通道（或 retry 已耗尽）。
```bash
wf status <task> --json              # 看 retry_count 和 last_feedback
# 修复问题后:
wf reset --step <task>               # 重试当前步
# 或者从头来:
wf start <task> --reset
```

### 场景 5: in_window 步骤监控
in_window 步骤在 tmux 窗口中运行（通常是 AI agent 在里面开发）。
```bash
wf capture <task>                    # 非侵入式查看窗口内容
wf enter <task>                      # 直接进入窗口交互 (Ctrl-B d 退出)
# agent 完成工作后:
wf done <task> -m "development complete"
```

## 状态决策速查表

根据 `wf list` 或 `wf status --json` 的输出，快速确定下一步操作：

```
status     | message        | 你的操作
-----------+----------------+----------------------------------------
pending    | -              | wf start <task> (检查 blocked_by 先)
running    | -              | 无需干预 (wf capture 监控 in_window)
waiting    | gate           | wf done <task> (确认准入条件)
waiting    | verify_human   | 审查产物 → wf done 或 wf reset --step
waiting    | on_fail_human  | 分析反馈 → wf done(放行)/reset --step(重试)/stop(放弃)
failed     | exit code/msg  | wf status --json 看 last_feedback → 修复 → wf reset --step
stopped    | -              | wf start --reset (重头来) 或 wf reset --step (续)
completed  | -              | 完成，无需操作
```

## 多任务管理

### 并发启动
```bash
wf start task-a &
wf start task-b &
wait                                 # 两个 task 并行推进直到各自遇到 yield 点
```

### 优先级判断
用 `wf list` 的输出来排优先级：
- `waiting` + `needs decision` → **最高**，有 task 被阻塞等你
- `waiting` + `gate` → **高**，检查准入条件后放行
- `waiting` + `needs review` → **高**，审查后放行
- `failed` → **中**，分析是否值得修复
- `running` → **低**，正常执行中，不需要干预

### 依赖管理
Task 可以有 `depends` 字段。被依赖的 task 必须先 Completed。
```bash
wf list                              # blocked task 的 INFO 列显示 "waiting: dep-task"
```

## 注意事项

### window_lost 是被动检测
tmux 窗口消失（进程崩溃、手动 kill）时，wf **不会主动通知你**。只有在你调用 `wf status`、`wf list`、`wf wait` 时才会检测到并标记为 Failed。所以：
- 对 in_window 步骤，定期 `wf list` 或 `wf capture` 检查健康
- 或者用 `wf wait <task> --until completed,failed,waiting` 阻塞等待状态变化

### Event Hook 是异步的
config 中的 `on` 字段定义的 hook 是 fire-and-forget。它们：
- 不保证执行顺序
- 失败不会影响主流程
- 可用于写日志、发通知，但不要依赖它们做决策

### 步骤索引
- **CLI 人类可读输出**: 1-based (`[1/8] setup`)
- **所有编程接口**: 0-based (`--step 0`、`--json` 输出、JSONL 事件、环境变量 `WF_STEP_INDEX`)

### wf done 的双重语义
- 对 **Waiting** 状态: 等同于 approve，发 StepApproved 事件，步骤前进
- 对 **Running + in_window** 状态: 等同于标记完成，触发 verify 流程，可能继续或失败

### AI Worker 辅助
`.wf/lib/ai-helpers.sh` 提供了 AI worker 包装函数：
- `extract_session_id` — 从日志提取 session ID 用于续接
- `extract_feedback` — 提取失败反馈
- `run_ai_worker` — 自动判断新建/续接会话

## 典型 Workflow 模板

```jsonc
{
  "workflow": [
    { "name": "setup",    "run": "git branch ${branch} ${base_branch} && git worktree add ${worktree} ${branch}" },
    { "name": "develop",  "run": "your-ai-agent-command", "in_window": true },
    { "name": "review",   "run": "echo 'Review changes in ${worktree}'", "verify": "human" },
    { "name": "test",     "run": "cd ${worktree} && make test", "on_fail": "retry", "max_retries": 2 },
    { "name": "merge",    "run": "cd ${repo_root} && git merge ${branch}" },
    { "name": "cleanup",  "run": "git worktree remove ${worktree} --force; git branch -D ${branch}; true" }
  ]
}
```

## 诊断技巧

```bash
# 看一个 task 的完整事件历史 (调试用)
wf events <task> | jq .

# 看当前步骤的 retry 次数和最后反馈
wf status <task> --json | jq '{step: .current_step, retry: .retry_count, feedback: .last_feedback}'

# 看 event hook 是否正常触发 (如果配置了写文件的 hook)
cat e2e-hook.log | tail -20
```

## JSON 输出格式参考

### `wf status --json` (无 task 参数 — 列表模式)

返回所有任务的摘要数组：

```json
[{
  "name": "my-task",
  "status": "waiting",         // pending|running|waiting|completed|failed|stopped
  "current_step": 2,           // 0-based
  "total_steps": 6,
  "step_name": "review",       // 完成时 "Done"，未启动时 "--"
  "message": "verify_human",   // 可选: gate|verify_human|on_fail_human (Waiting); 失败信息 (Failed)
  "started_at": "RFC3339",     // 可选
  "updated_at": "RFC3339",     // 可选
  "blocked_by": ["dep-task"],  // 空时省略
  "retry_count": 0,            // 仅自动重试 (auto=true 的 StepReset)
  "last_feedback": "string"    // 可选: 最近 StepCompleted(exit!=0) 的 stdout+stderr
}]
```

### `wf status <task> --json` (单任务详情)

在列表模式基础上增加 `description`、`depends`、`workflow` 字段，去掉 `step_name`：

```json
{
  "name": "my-task",
  "description": "Markdown 描述",
  "depends": ["setup-infra"],
  "status": "running",
  "current_step": 3, "total_steps": 6,
  "retry_count": 1,
  "last_feedback": "Error: connection refused",
  "workflow": [
    { "index": 0, "name": "setup",   "status": "success" },
    { "index": 1, "name": "develop", "step_type": "in_window", "status": "success" },
    { "index": 2, "name": "review",  "status": "skipped" },
    { "index": 3, "name": "test",    "status": "current" },
    { "index": 4, "name": "merge",   "status": "pending" },
    { "index": 5, "name": "cleanup", "status": "pending" }
  ]
}
```

**StepInfo**: `index`(0-based), `name`, `step_type`(`"gate"`/`"in_window"`/省略), `status`(`success`/`failed`/`skipped`/`current`/`pending`)

### 字段行为要点

- `retry_count` 只统计自动重试 (`StepReset` 中 `auto=true`)，不含手动 `wf reset --step`
- `last_feedback` 倒序搜索，遇 `TaskReset` 或手动 `StepReset` 停止
- 可选字段 (`message`/`started_at`/`updated_at`/`last_feedback`) null 时省略；`blocked_by` 空数组时省略

## 故障排查

| 症状 | 原因 | 解决方法 |
|------|------|----------|
| tmux session 找不到 | session 不存在 | `tmux ls` 检查；`tmux new-session -d -s <session>` 创建 |
| "Task already running" | 另一个 wf start 在运行 | `wf status <task>` 确认；`wf stop && wf start` 重启 |
| worktree 已存在 | 上次运行残留 | `git worktree remove .wf/worktrees/<task> --force && git branch -D wf/<task>` 后 `wf reset` |
| JSONL 损坏 | 写入中断 | `tail -1 .wf/logs/<task>.jsonl \| jq .` 检查；`wf reset` 重置 |
| window_lost 但进程在 | tmux 窗口命名冲突 | `tmux list-windows -t <session>` 检查；`wf reset --step` 重试 |
| "Not a wf project" | 缺少 .wf 目录 | `wf init` 初始化 |
| 依赖阻塞 | 前置任务未完成 | `wf list` 查看阻塞来源，优先完成阻塞任务 |
