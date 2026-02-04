# Config Reference

> **v1 参考**：
> JSONC 解析 `/Users/yansir/code/tmp/worktree/src/models/config.rs:148-170`
> 默认 config 模板 `/Users/yansir/code/tmp/worktree/src/commands/init/config.rs`
> .gitignore 模板 `/Users/yansir/code/tmp/worktree/src/commands/init/templates.rs:7-16`

## 文件位置

```
.wf/config.jsonc
```

支持 JSONC（带注释的 JSON）。

## 完整示例

```jsonc
{
  // ============================================
  // 基础配置
  // ============================================

  // tmux session 名称
  // 默认: 项目目录名
  "session": "my-project",

  // Terminal multiplexer
  // 用于 wf stop (发送 Ctrl+C), wf enter (切换窗口)
  // 默认: "tmux"
  // MVP 只支持 tmux
  "multiplexer": "tmux",

  // Claude CLI 命令路径
  // 默认: "claude"
  // "claude_command": "claude",

  // Worktree 存放目录（相对于 repo root）
  // 默认: ".wf/worktrees"
  // "worktree_dir": ".wf/worktrees",

  // ============================================
  // Workflow
  // ============================================
  // 所有任务共享的执行流程
  // 支持变量: ${task}, ${branch}, ${worktree}, ${window},
  //          ${session}, ${repo_root}, ${step}

  "workflow": [
    // 创建资源
    { "name": "Create branch", "run": "git branch ${branch}" },
    { "name": "Create worktree", "run": "git worktree add ${worktree} ${branch}" },
    { "name": "Install deps", "run": "cd ${worktree} && bun i" },
    { "name": "Create window", "run": "tmux new-window -t ${session} -n ${window} -c ${worktree}" },

    // 开发
    {
      "name": "Develop",
      "run": "claude -p --model sonnet '@.wf/tasks/${task}.md'",
      "in_window": true
    },

    // 人工确认开发完成
    { "name": "Review development" },

    // 验证
    { "name": "Type check", "run": "cd ${worktree} && bun typecheck" },
    { "name": "Lint", "run": "cd ${worktree} && bun check" },

    // 合并
    {
      "name": "Merge",
      "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge'"
    },

    // 清理资源
    {
      "name": "Cleanup",
      "run": "tmux kill-window -t ${session}:${window} 2>/dev/null; git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true"
    }
  ],

  // ============================================
  // Hooks (可选)
  // ============================================
  // 事件触发的 shell 命令，fire-and-forget

  "hooks": {
    "task.completed": "terminal-notifier -title 'wf' -message '${task} completed'",
    "step.failed": "terminal-notifier -title 'wf' -message '${task}: ${step} failed'"
  }
}
```

## 字段说明

### 顶层字段

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `session` | string | 项目目录名 | tmux session 名称 |
| `multiplexer` | string | `"tmux"` | multiplexer 类型 |
| `claude_command` | string | `"claude"` | Claude CLI 路径 |
| `worktree_dir` | string | `".wf/worktrees"` | worktree 存放目录 |
| `workflow` | Step[] | **必填** | workflow 定义 |
| `hooks` | object | `{}` | 事件 hook 定义 |

### Step

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | 是 | step 名称，用于显示和 log 文件命名 |
| `run` | string | 否 | shell 命令。省略则为 checkpoint |
| `in_window` | bool | 否 | 是否发送到 tmux window 执行 |

### Hooks

key 是事件名，value 是 shell 命令。

| 事件 | 触发时机 |
|------|---------|
| `task.started` | 任务开始 |
| `task.completed` | 任务完成 |
| `task.failed` | step 失败 |
| `step.success` | 任意 step 成功 |
| `step.failed` | 任意 step 失败 |
| `step.blocked` | step 被标记 blocked |
| `checkpoint` | 到达 checkpoint |

## 变量

所有变量在 step 的 `run` 和 hook 命令中可用。

| 变量 | 值 | 示例 |
|------|------|------|
| `${task}` | 任务名 | `auth` |
| `${branch}` | `wf/{task}` | `wf/auth` |
| `${worktree}` | `{repo_root}/{worktree_dir}/{task}` | `/path/to/project/.wf/worktrees/auth` |
| `${window}` | `{task}` | `auth` |
| `${session}` | config.session | `my-project` |
| `${repo_root}` | git repo 根目录 | `/path/to/project` |
| `${step}` | 当前 step name | `Type check` |

## 最小配置

```jsonc
{
  "workflow": [
    { "name": "Create branch", "run": "git branch wf/${task}" },
    { "name": "Create worktree", "run": "git worktree add .wf/worktrees/${task} wf/${task}" },
    { "name": "Develop", "run": "claude '@.wf/tasks/${task}.md'", "in_window": true },
    { "name": "Cleanup", "run": "git worktree remove .wf/worktrees/${task} --force; git branch -D wf/${task}" }
  ]
}
```
