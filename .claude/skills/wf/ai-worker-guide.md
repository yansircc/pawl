# AI Worker 集成指南

本文档面向在 wf 编排器中集成 AI 编码代理 (Worker) 的开发者。Worker 运行在 `in_window` 步骤的 tmux 窗口中，由 Foreman 通过 wf CLI 管理生命周期。

## 1. AI Worker 概述

Worker 是执行实际编码工作的 AI 代理。典型流程：

```
wf start task
  → setup (sync)
  → develop (in_window)    ← Worker 在此运行
  → verify (sync/human)
  → merge (sync)
  → cleanup (sync)
```

Worker 运行在 tmux 窗口中，享有完整的终端环境。wf 提供 `.wf/lib/ai-helpers.sh` 辅助库处理会话管理、失败续接等常见模式。

## 2. ai-helpers.sh 函数参考

### extract_session_id(jsonl_file)

从 JSONL 日志中提取最近一次的 `session_id`，用于会话续接。

```bash
source "$WF_REPO_ROOT/.wf/lib/ai-helpers.sh"
sid=$(extract_session_id "$WF_LOG_FILE")
# 返回空字符串表示无历史会话（首次运行）
```

实现原理：在 JSONL 中查找最后一个包含 `session_id` 的记录。Claude 非交互模式的 stdout 会包含 session_id，被 wf 捕获到 JSONL 中。

### extract_feedback(jsonl_file, [step_index])

从 `step_completed` 事件中提取失败反馈（`exit_code != 0` 的 stderr）。

```bash
# 提取最近一次失败反馈
feedback=$(extract_feedback "$WF_LOG_FILE")

# 按步骤索引过滤（0-based）
feedback=$(extract_feedback "$WF_LOG_FILE" 1)
```

用途：重试时将上一次失败原因注入给 Worker，使其能针对性修复。

### run_ai_worker([options])

一站式 Worker 启动函数，自动判断新建 vs 续接会话。

| 选项 | 默认值 | 说明 |
|------|--------|------|
| `--log-file <path>` | `$WF_LOG_FILE` | JSONL 日志路径 |
| `--task-file <path>` | `$WF_TASK_FILE` | Task markdown 路径 |
| `--tools <tools>` | `Bash,Read,Write` | 逗号分隔的工具列表 |
| `--claude-cmd <cmd>` | `claude` | Claude CLI 命令 |
| `--extra-args <args>` | (空) | 传递给 claude 的额外参数 |

行为逻辑：
1. 调用 `extract_session_id` 检查是否有历史会话
2. **无 session_id** → 首次运行：`cat task.md | claude -p - --tools ...`
3. **有 session_id** → 续接：提取 feedback，用 `claude -p "Fix: $feedback" -r $session_id` 继续

## 3. 自定义 Wrapper 脚本

当 `run_ai_worker` 的默认行为不满足需求时，编写自定义 wrapper：

```bash
#!/bin/bash
# .wf/lib/my-wrapper.sh
source "$WF_REPO_ROOT/.wf/lib/ai-helpers.sh"

cd "$WF_WORKTREE"

# 自定义工具集和模型
run_ai_worker \
  --tools "Bash,Read,Write,Edit" \
  --extra-args "--model sonnet"
```

在 config 中引用：

```jsonc
{
  "name": "develop",
  "run": "bash ${repo_root}/.wf/lib/my-wrapper.sh",
  "in_window": true
}
```

更精细的控制——绕过 `run_ai_worker`，直接调用 claude：

```bash
#!/bin/bash
source "$WF_REPO_ROOT/.wf/lib/ai-helpers.sh"
cd "$WF_WORKTREE"

sid=$(extract_session_id "$WF_LOG_FILE")
if [ -n "$sid" ]; then
    feedback=$(extract_feedback "$WF_LOG_FILE")
    claude -p "Previous error: $feedback. Fix it." -r "$sid" --tools "Bash,Read,Write"
else
    # 首次运行：拼接系统 prompt + task 内容
    {
      echo "You are working in: $WF_WORKTREE"
      echo "Branch: $WF_BRANCH"
      echo "---"
      cat "$WF_TASK_FILE"
    } | claude -p - --tools "Bash,Read,Write,Edit" --model sonnet
fi
```

## 4. Claude 非交互模式

`claude -p` 是 Worker 的基础。关键用法：

```bash
cat task.md | claude -p - --tools "Bash,Read,Write"   # 管道注入 task
claude -p "Fix the bug" -r $session_id                # 续接会话
claude -p "task" --model sonnet --output-format json   # 指定模型和输出格式
```

`-p` 为非交互模式，`-r` 在同一上下文中继续（保留代码理解），`--tools` 控制可用工具集。

## 5. 会话续接流程

续接是 Worker 效率的关键——避免每次重试都从零开始理解代码。

```
Round 1 (首次运行):
  cat task.md | claude -p - --tools "Bash,Read,Write"
    → Worker 执行开发工作
    → stdout 包含 session_id
    → wf 将 stdout 捕获到 JSONL

  wf done <task>
    → verify 脚本检查 → 失败
    → StepCompleted(exit_code=1, stderr="test failed: ...")
    → on_fail: "retry" → StepReset(auto=true)
    → 步骤重新执行

Round 2 (自动重试):
  run_ai_worker 内部:
    → extract_session_id() 从 JSONL 获取 session_id
    → extract_feedback() 获取 "test failed: ..."
    → claude -p "Previous attempt failed verification. Feedback: test failed: .... Please fix and try again." -r $session_id
    → Worker 在同一上下文中继续修复
```

## 6. Event Hook 通知闭环

配置 event hook 让 Foreman 实时收到 Worker 状态通知：

```jsonc
{
  "on": {
    "step_completed": "mkdir -p /tmp/wf-notify.lock 2>/dev/null && tmux send-keys -t ${session}:foreman '[wf] ${task} step ${step} done (exit=${exit_code})' Enter; rmdir /tmp/wf-notify.lock 2>/dev/null; true",
    "step_waiting": "tmux send-keys -t ${session}:foreman '[wf] ${task} waiting: ${reason}' Enter; true"
  }
}
```

机制解释：
- `mkdir -p /tmp/wf-notify.lock` — 利用 mkdir 原子性作互斥锁，防止并发 hook 交错
- `tmux send-keys -t ${session}:foreman` — 向 Foreman 所在窗口注入通知文本
- `rmdir` 释放锁；尾部 `; true` 保证 hook 不因锁竞争失败而报错
- Hook 是 fire-and-forget，不影响主流程

Event hook 中可使用事件特定变量：
- `step_completed`: `${exit_code}`, `${duration}`
- `step_waiting`: `${reason}` (gate / verify_human / on_fail_human)
- `step_reset`: `${auto}` (true / false)

## 7. 环境变量

Worker 进程中所有 `WF_*` 环境变量均可用。完整变量表见 SKILL.md 的"变量"章节。

## 8. 实战配置示例

### 基础 AI 开发步骤

```jsonc
{
  "name": "develop",
  "run": "source ${repo_root}/.wf/lib/ai-helpers.sh && cd ${worktree} && run_ai_worker",
  "in_window": true
}
```

### 带自定义工具和模型

```jsonc
{
  "name": "develop",
  "run": "source ${repo_root}/.wf/lib/ai-helpers.sh && cd ${worktree} && run_ai_worker --tools 'Bash,Read,Write,Edit,Grep' --extra-args '--model sonnet'",
  "in_window": true
}
```

### 开发 + 自动验证 + 重试

```jsonc
{
  "name": "develop",
  "run": "source ${repo_root}/.wf/lib/ai-helpers.sh && cd ${worktree} && run_ai_worker",
  "in_window": true,
  "verify": "cd ${worktree} && npm test",
  "on_fail": "retry",
  "max_retries": 3
}
```

流程：Worker 完成 → Foreman `wf done` → verify 脚本运行测试 → 失败则自动重试（Worker 续接会话修复）→ 最多 3 次。

### 自定义 Wrapper + 人工审核

```jsonc
{
  "name": "develop",
  "run": "bash ${repo_root}/.wf/lib/my-wrapper.sh",
  "in_window": true,
  "verify": "human"
}
```

流程：Worker 完成 → Foreman `wf done` → 进入 Waiting(verify_human) → Foreman 审查后再次 `wf done` 放行。

### 完整 Workflow 模板

```jsonc
{
  "session": "my-project",
  "workflow": [
    { "name": "setup",   "run": "git branch ${branch} ${base_branch} 2>/dev/null; git worktree add ${worktree} ${branch}" },
    { "name": "develop", "run": "source ${repo_root}/.wf/lib/ai-helpers.sh && cd ${worktree} && run_ai_worker", "in_window": true, "verify": "cd ${worktree} && make test", "on_fail": "retry", "max_retries": 3 },
    { "name": "review",  "run": "echo 'Review changes in ${worktree}'", "verify": "human" },
    { "name": "merge",   "run": "cd ${repo_root} && git merge ${branch}" },
    { "name": "cleanup", "run": "git worktree remove ${worktree} --force; git branch -D ${branch}; true" }
  ],
  "on": {
    "step_completed": "mkdir -p /tmp/wf-notify.lock 2>/dev/null && tmux send-keys -t ${session}:foreman '[wf] ${task} step ${step} done (exit=${exit_code})' Enter; rmdir /tmp/wf-notify.lock 2>/dev/null; true"
  }
}
```
