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

## Config 设计规则

生成或修改 `.wf/config.jsonc` 时，必须遵守：

1. **每个可失败的 in_window 步骤必须定义 `on_fail`**（"retry" 或 "human"），否则失败即终态
2. **每个有可观测产出的步骤必须定义 `verify`**，否则 `wf done` 无条件信任
3. **in_window 步骤的 `run` 必须 `cd ${worktree}`**，否则 worker 在错误目录执行

## Config (.wf/config.jsonc)

```jsonc
{
  "session": "my-project",      // tmux session 名 (默认: 目录名)
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

所有 `run`/`verify`/hook 命令中可用 `${var}`，子进程中为 `WF_VAR` 环境变量：

| 变量 | 值 |
|------|-----|
| `${task}` / `${branch}` | 任务名 / `wf/{task}` |
| `${worktree}` | `{repo_root}/{worktree_dir}/{task}` |
| `${session}` / `${window}` | tmux session 名 / 同 task 名 |
| `${repo_root}` | 仓库根目录 |
| `${step}` / `${step_index}` | 当前步骤名 / 索引 (0-based) |
| `${base_branch}` | 基础分支 |
| `${log_file}` / `${task_file}` | `.wf/logs/{task}.jsonl` / `.wf/tasks/{task}.md` |

## Event Hooks

在 config 的 `"on"` 字段配置，fire-and-forget 异步执行：

可用事件: `task_started`, `step_completed`(+`${exit_code}`,`${duration}`), `step_waiting`(+`${reason}`), `step_approved`, `step_skipped`, `step_reset`(+`${auto}`), `window_launched`, `window_lost`, `task_stopped`, `task_reset`

## Claude CLI 与 wf 集成

Worker 通常通过 `claude -p`（非交互模式）运行在 in_window 步骤中：

```bash
# 基础: 管道注入 task.md 作为 prompt
cat ${task_file} | claude -p - --tools "Bash,Read,Write"

# 续接会话: -r 保留上下文 (避免重头理解代码)
claude -p "Fix: $feedback" -r $session_id --tools "Bash,Read,Write"

# 结构化输出: 机器可解析的结果
claude -p "task" --output-format json --json-schema '{"type":"object",...}'
```

关键 flag: `-p`(非交互), `-r session_id`(续接), `--tools`(可用工具集), `--output-format json`(JSON 信封含 session_id)

`.wf/lib/ai-helpers.sh` 提供一站式封装：

| 函数 | 用途 |
|------|------|
| `extract_session_id <jsonl>` | 从 JSONL 提取最近 session_id |
| `extract_feedback <jsonl> [step]` | 提取失败反馈 (exit!=0 的 stderr) |
| `run_ai_worker [--tools T] [--extra-args A]` | 自动判断新建/续接，注入 feedback |

典型 in_window 步骤: `"run": "source ${repo_root}/.wf/lib/ai-helpers.sh && cd ${worktree} && run_ai_worker"`

## 深入参考

当需要 JSON schema、故障排查等详细信息时，读取 `reference.md`。
