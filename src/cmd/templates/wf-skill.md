# wf — AI Agent Orchestrator

wf 是一个**可恢复的协程编排器**：沿固定步骤序列推进 AI coding agent，遇到无法自决的节点暂停 (yield)，崩溃后从 append-only 日志重建状态。每个 task 在独立的 git worktree 中执行，通过 tmux 窗口管理长时间运行的 AI worker。

## CLI 命令

| 命令 | 说明 |
|------|------|
| `wf init` | 初始化项目 (创建 .wf/ 和 .claude/skills/wf/) |
| `wf create <name> [desc] [--depends a,b]` | 创建任务 |
| `wf list` | 列出所有任务状态 |
| `wf start <task> [--reset]` | 启动任务 (--reset 先重置) |
| `wf status [task] [--json]` | 查看状态 (--json 用 0-based 索引) |
| `wf stop <task>` | 停止任务 |
| `wf reset <task>` | 完全重置任务 |
| `wf reset --step <task>` | 重试当前步骤 |
| `wf done <task> [-m msg]` | 放行 / 审核通过 / 标记完成 |
| `wf enter <task>` | 附加到 tmux 窗口 |
| `wf capture <task> [-l N] [--json]` | 捕获 tmux 窗口内容 |
| `wf wait <task> --until <status> [-t sec]` | 等待指定状态 |
| `wf log <task> [--step N] [--all] [--all-runs]` | 查看日志 |
| `wf events [task] [--follow]` | 原始事件流 |

## Step 类型速查

每个 step 有 4 个正交属性：`run`、`verify`、`on_fail`、`in_window`

| 类型 | 配置 | 行为 |
|------|------|------|
| 普通步骤 | `"run": "cmd"` | 同步执行，exit 0 前进，否则 Failed |
| Gate | 无 `run` | 立即暂停，等 `wf done` 放行 |
| 人工审核 | `"verify": "human"` | 运行后暂停等人工审查 |
| 自动验证 | `"verify": "test.sh"` | 运行后执行验证脚本 |
| 自动重试 | `"on_fail": "retry"` | 失败自动重试 (max_retries, 默认 3) |
| 人工介入 | `"on_fail": "human"` | 失败暂停等人工决策 |
| 窗口任务 | `"in_window": true` | tmux 窗口中执行，等 `wf done` |

## 状态机

```
Pending → Running → Waiting    (等 wf done)
                  → Completed  (全部步骤完成)
                  → Failed     (步骤失败 / 窗口丢失)
                  → Stopped    (wf stop)
```

- `wf reset` 可从任何非 Pending 状态回到初始
- `wf reset --step` 重试当前步骤 (Waiting/Failed/Stopped)

## Config (.wf/config.jsonc)

```jsonc
{
  "session": "my-project",      // tmux session 名 (默认: 目录名)
  "worktree_dir": ".wf/worktrees", // worktree 目录 (默认)
  "base_branch": "main",        // 基础分支 (默认)
  "workflow": [                  // 步骤序列 (必须)
    { "name": "step-name", "run": "cmd", "verify": "human|script", "on_fail": "retry|human", "in_window": true, "max_retries": 3 }
  ],
  "on": { "event_name": "shell command" }  // Event hooks (可选)
}
```

## Task 定义 (.wf/tasks/{task}.md)

```yaml
---
name: my-task
depends: [other-task]    # 依赖 (可选)
skip: [cleanup]          # 跳过步骤 (可选)
---

Markdown 描述 (同时作为 AI Worker 的 system prompt)
```

## 变量

所有 `run`/`verify` 命令中可用 `${var}`，子进程中为 `WF_VAR` 环境变量：

| 变量 | 环境变量 | 值 |
|------|---------|-----|
| `${task}` | `WF_TASK` | 任务名 |
| `${branch}` | `WF_BRANCH` | `wf/{task}` |
| `${worktree}` | `WF_WORKTREE` | `{repo_root}/{worktree_dir}/{task}` |
| `${window}` | `WF_WINDOW` | 同 task 名 |
| `${session}` | `WF_SESSION` | tmux session 名 |
| `${repo_root}` | `WF_REPO_ROOT` | 仓库根目录 |
| `${step}` | `WF_STEP` | 当前步骤名 |
| `${step_index}` | `WF_STEP_INDEX` | 步骤索引 (0-based) |
| `${base_branch}` | `WF_BASE_BRANCH` | 基础分支 |
| `${log_file}` | `WF_LOG_FILE` | `.wf/logs/{task}.jsonl` |
| `${task_file}` | `WF_TASK_FILE` | `.wf/tasks/{task}.md` |

## Event Hooks

在 config 的 `"on"` 字段配置，fire-and-forget 异步执行：

可用事件: `task_started`, `step_completed`(+`${exit_code}`,`${duration}`), `step_waiting`(+`${reason}`), `step_approved`, `step_skipped`, `step_reset`(+`${auto}`), `window_launched`, `window_lost`, `task_stopped`, `task_reset`

```jsonc
"on": { "step_completed": "echo '[${task}] ${step} exit=${exit_code}'" }
```

## 深入参考

当需要更详细的指南时，读取以下文件：

| 文件 | 何时读取 |
|------|---------|
| `foreman-guide.md` | 作为 Foreman 管理多任务时 (决策场景、JSON schema、故障排查) |
| `task-authoring-guide.md` | 创建/修改 Task.md 时 (五要素写作法、迭代反馈、完整示例) |
| `ai-worker-guide.md` | 配置 AI Worker / in_window 步骤时 (ai-helpers.sh、wrapper.sh、会话续接) |
| `.wf/lib/ai-helpers.sh` | Worker 辅助函数 (extract_session_id, run_ai_worker) |
