# Data Model

## 目录结构

```
project/
├── .wf/
│   ├── config.jsonc          # workflow + hooks 配置
│   ├── status.json           # 运行时状态（自动管理）
│   ├── tasks/                # 任务定义
│   │   ├── auth.md
│   │   ├── dashboard.md
│   │   └── settings.md
│   ├── logs/                 # step 日志
│   │   ├── auth/
│   │   │   ├── step-0-create-branch.log
│   │   │   ├── step-1-create-worktree.log
│   │   │   └── ...
│   │   └── dashboard/
│   │       └── ...
│   └── worktrees/            # git worktrees（gitignore）
│       ├── auth/
│       └── dashboard/
├── .gitignore                # 含 .wf/worktrees, .wf/status.json, .wf/logs
└── ...
```

## Task 定义

> **v1 参考**：
> YAML frontmatter 解析 `/Users/yansir/code/tmp/worktree/src/models/task_parser.rs:21-58`
> 任务名验证 `/Users/yansir/code/tmp/worktree/src/models/task_parser.rs:60-109`

文件：`.wf/tasks/{name}.md`

```yaml
---
name: auth
depends:
  - database
  - config
---

## 任务描述

实现用户认证功能：
- 登录/注册 API
- JWT token 管理
- 中间件保护路由

## 验收标准

- [ ] POST /api/auth/login 返回 JWT token
- [ ] 中间件验证 token 有效性
- [ ] 单元测试覆盖
```

### Frontmatter 字段

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | 是 | 任务名，必须是合法 git 分支名后缀 |
| `depends` | string[] | 否 | 依赖的任务名列表 |

## 状态存储

> **v1 参考**：
> 状态存储和原子写入 `/Users/yansir/code/tmp/worktree/src/models/status.rs`
> JSONC 解析模式 `/Users/yansir/code/tmp/worktree/src/models/config.rs:148-170`

文件：`.wf/status.json`

由 wf 自动管理，不要手动编辑（除非需要恢复）。

```json
{
  "tasks": {
    "auth": {
      "current_step": 5,
      "status": "waiting",
      "started_at": "2024-01-01T10:00:00Z",
      "updated_at": "2024-01-01T10:15:00Z",
      "step_status": "success",
      "message": null
    },
    "dashboard": {
      "current_step": 3,
      "status": "failed",
      "started_at": "2024-01-01T10:02:00Z",
      "updated_at": "2024-01-01T10:07:00Z",
      "step_status": "failed",
      "message": "exit code 1"
    },
    "settings": {
      "current_step": 5,
      "status": "running",
      "started_at": "2024-01-01T10:01:00Z",
      "updated_at": "2024-01-01T10:01:00Z",
      "step_status": null,
      "message": null
    }
  }
}
```

### Task State 字段

| 字段 | 类型 | 说明 |
|------|------|------|
| `current_step` | number | 当前 step 的 0-based index |
| `status` | string | 任务状态（见下方） |
| `started_at` | string | 任务启动时间 (ISO 8601) |
| `updated_at` | string | 最后状态更新时间 (ISO 8601) |
| `step_status` | string? | 当前 step 的结果：success/failed/blocked/skipped |
| `message` | string? | 失败/阻塞的原因 |

### Task Status

| 状态 | 含义 | 转换来源 |
|------|------|---------|
| `pending` | 未启动 | 初始状态 / `wf reset` |
| `running` | step 正在执行 | `wf start` / `wf retry` / `wf next` |
| `waiting` | 等待条件 | checkpoint / `in_window` step 完成 / blocked |
| `completed` | workflow 全部完成 | 最后一个 step 成功 |
| `failed` | step 失败 | step exit 非 0 / `wf fail` |
| `stopped` | 被手动停止 | `wf stop` |

### 状态转换图

```
                 wf start
  pending ─────────────────→ running
                                │
                    ┌───────────┼───────────┐
                    ↓           ↓           ↓
                 waiting     failed      completed
                    │           │
         wf next    │  wf retry │
         wf retry   │           │
                    ↓           ↓
                 running ←──────┘
                    │
          wf stop   │
                    ↓
                 stopped
                    │
          wf retry  │
                    ↓
                 running
```

## 派生值

以下值从 task name + config 确定性派生，不需要存储：

```
task name:  "auth"
branch:     "wf/auth"
worktree:   "{repo_root}/.wf/worktrees/auth"
window:     "auth"
session:    config.session
```

## Config 数据结构

```rust
struct Config {
    session: String,           // tmux session 名
    multiplexer: String,       // "tmux"
    claude_command: String,    // "claude"
    worktree_dir: String,      // ".wf/worktrees"
    workflow: Vec<Step>,       // workflow 定义
    hooks: HashMap<String, String>,  // 事件 → 命令
}

struct Step {
    name: String,              // 显示名称
    run: Option<String>,       // shell 命令（None = checkpoint）
    in_window: bool,           // 是否在 window 中执行
}
```

## Task 数据结构

```rust
struct TaskDefinition {
    name: String,              // 任务名
    depends: Vec<String>,      // 依赖列表
    description: String,       // markdown 描述
}

struct TaskState {
    current_step: usize,       // 当前 step index
    status: TaskStatus,        // 任务状态
    started_at: DateTime,      // 启动时间
    updated_at: DateTime,      // 最后更新时间
    step_status: Option<StepStatus>,  // 当前 step 结果
    message: Option<String>,   // 失败/阻塞原因
}

enum TaskStatus {
    Pending,
    Running,
    Waiting,
    Completed,
    Failed,
    Stopped,
}

enum StepStatus {
    Success,
    Failed,
    Blocked,
    Skipped,
}
```

## Log 格式

文件：`.wf/logs/{task}/step-{N}-{slugified-name}.log`

文件名中的 name 做 slugify 处理：空格 → `-`，小写，去除特殊字符。

```
=== Step 6: Type check ===
Command: cd /Users/dev/project/.wf/worktrees/auth && bun typecheck
Started: 2024-01-01T10:05:00Z

src/auth/login.ts(15,3): error TS2345: Argument of type 'string' is not
  assignable to parameter of type 'number'.

Exit code: 1
Duration: 3.2s
Status: failed
```

对于 `in_window` 的 step：

```
=== Step 4: Develop ===
Command (in_window): claude -p --model sonnet '@.wf/tasks/auth.md'
Sent to window: my-project:auth
Started: 2024-01-01T10:03:00Z

Waiting for: wf done / wf fail / wf block

Completed: 2024-01-01T10:18:00Z
Duration: 15m
Status: success (wf done)
```

对于 checkpoint：

```
=== Step 5: Review development ===
Type: checkpoint
Waiting for: wf next
Started: 2024-01-01T10:18:00Z

Resumed: 2024-01-01T10:25:00Z
Wait duration: 7m
```

## 并发安全

> **v1 参考**：原子写入（temp + rename）`/Users/yansir/code/tmp/worktree/src/models/status.rs`

多个任务可能同时更新 `status.json`（例如两个 agent 同时调用 `wf done`）。

使用文件锁保证原子性：

```rust
fn update_status<F>(f: F) -> Result<()>
where F: FnOnce(&mut StatusStore) -> Result<()>
{
    let lock = FileLock::acquire(".wf/status.lock")?;
    let mut store = StatusStore::load(".wf/status.json")?;
    f(&mut store)?;
    store.save(".wf/status.json")?;
    lock.release();
    Ok(())
}
```
