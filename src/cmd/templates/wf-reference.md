# wf Reference — 详细参考

SKILL.md 的补充参考。仅在需要 JSON schema、故障排查、hook 模式时读取。

## wf status --json 输出

### 列表模式 (无 task 参数)

```json
[{
  "name": "my-task",
  "status": "waiting",         // pending|running|waiting|completed|failed|stopped
  "current_step": 2,           // 0-based
  "total_steps": 6,
  "step_name": "review",
  "message": "verify_human",   // gate|verify_human|on_fail_human (Waiting); 失败信息 (Failed)
  "started_at": "RFC3339",
  "updated_at": "RFC3339",
  "blocked_by": ["dep-task"],  // 空时省略
  "retry_count": 0,            // 仅自动重试 (StepReset auto=true)
  "last_feedback": "string"    // 最近 StepCompleted(exit!=0) 的 stdout+stderr
}]
```

### 单任务详情 (有 task 参数)

在列表模式基础上增加 `description`、`depends`、`workflow` 字段：

```json
{
  "workflow": [
    { "index": 0, "name": "setup", "status": "success" },
    { "index": 1, "name": "develop", "step_type": "in_window", "status": "current" },
    { "index": 2, "name": "review", "step_type": "gate", "status": "pending" }
  ]
}
```

`step_type`: `"gate"` / `"in_window"` / 省略。`status`: `success` / `failed` / `skipped` / `current` / `pending`。

## ai-helpers.sh 行为详解

`run_ai_worker` 的决策流程：

```
1. extract_session_id($WF_LOG_FILE)
   └→ grep JSONL 中最后一个 session_id 字段
2. 有 session_id?
   ├→ 有: extract_feedback → claude -p "Fix: $feedback" -r $session_id
   └→ 无: cat $WF_TASK_FILE | claude -p - --tools "Bash,Read,Write"
```

约束：`-r session_id` 必须在同一 cwd（session 数据按项目目录存储）。wf 的 worktree 路径是确定的，满足此约束。

## Event Hook 通知模式

### 写日志文件 (最简)

```jsonc
"on": { "step_completed": "echo '[${task}] ${step} exit=${exit_code}' >> ${repo_root}/.wf/hook.log" }
```

### tmux 通知 Foreman (交互模式)

```jsonc
"on": {
  "step_completed": "mkdir /tmp/wf-notify.lock 2>/dev/null && tmux send-keys -t ${session}:foreman -l '[wf] ${task}/${step} done (exit=${exit_code})' && tmux send-keys -t ${session}:foreman C-Enter && sleep 0.3 && rmdir /tmp/wf-notify.lock; true"
}
```

要点：`mkdir` 原子互斥锁防并发交错；`-l` 发送字面文本；`C-Enter` 提交 Claude Code TUI 输入；`sleep 0.3` 确保原子性。

## 故障排查

| 症状 | 原因 | 解决 |
|------|------|------|
| tmux session 找不到 | session 不存在 | `tmux new-session -d -s <session>` |
| "Task already running" | 另一个 wf start 在运行 | `wf stop <task> && wf start <task>` |
| worktree 已存在 | 上次运行残留 | `git worktree remove .wf/worktrees/<task> --force && git branch -D wf/<task>` 后 `wf reset` |
| window_lost 但进程在 | tmux 窗口命名冲突 | `tmux list-windows -t <session>` 检查 |
| 依赖阻塞 | 前置任务未完成 | `wf list` 查看阻塞来源 |
| `-r session_id` 失败 | cwd 不匹配 | 必须在同一 worktree 目录下执行 |

## 步骤索引约定

- **CLI 人类可读输出**: 1-based (`[1/8] setup`)
- **所有编程接口**: 0-based (`--step 0`、`--json` 输出、JSONL 事件、`WF_STEP_INDEX`)
