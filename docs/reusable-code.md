# Reusable Code Reference

v1 项目 (`/Users/yansir/code/tmp/worktree`) 中可复用的代码和逻辑。

---

## 1. Agent 自验证机制 (Stop Hook)

**最重要的可复用逻辑**。让 agent 在退出前自动验证并标记状态。

### 完整链路

```
Agent 尝试退出
  → Claude CLI 触发 Stop Hook
  → node verify-stop.cjs
  → 解析 transcript，检查是否调用了 wf done/fail/block
  → 未调用 → block 退出，提示 agent 阅读 verify.md
  → agent 自检 → 调用 wf done/fail/block
  → 再次 Stop Hook → 检测到调用 → 放行
```

### 关键文件

| 文件 | 说明 |
|------|------|
| `/Users/yansir/code/tmp/worktree/src/commands/init/templates.rs:68-173` | Stop Hook 脚本 (`verify-stop.cjs`)，核心检测逻辑 |
| `/Users/yansir/code/tmp/worktree/src/commands/init/templates.rs:18-48` | 验证清单模板 (`verify.md`) |
| `/Users/yansir/code/tmp/worktree/src/commands/init/templates.rs:50-65` | Claude settings 模板 (`verify-settings.json`) |

### 关键逻辑 (verify-stop.cjs)

```javascript
// 1. 读取 transcript，扫描所有 tool_use
// 2. 检测 Bash 工具调用中是否匹配 /wf\s+step\s+(done|block|fail)/
// 3. 防止无限循环：状态文件 /tmp/wf-verify-${sessionId}.json
//    - 首次触发：记录 lastPromptedLine，block 退出
//    - 二次触发：放行（即使没调用也放行，防止死循环）
// 4. 已调用 wf step：清理状态文件，允许退出
```

### v2 适配

在 v2 中，agent 自验证的命令从 `wf step done/fail/block` 简化为 `wf done/fail/block`。
需要修改 Stop Hook 中的正则：

```diff
- if (/wf\s+step\s+(done|block|fail)/.test(command))
+ if (/wf\s+(done|block|fail)/.test(command))
```

---

## 2. YAML Frontmatter 解析

### 关键文件

| 文件 | 说明 |
|------|------|
| `/Users/yansir/code/tmp/worktree/src/models/task_parser.rs:21-58` | `parse_markdown()` 解析逻辑 |
| `/Users/yansir/code/tmp/worktree/src/models/task_parser.rs:60-109` | `validate_name()` 任务名验证 |

### 解析逻辑

```rust
// 输入: "---\nname: auth\ndepends:\n  - db\n---\n任务描述..."
// 输出: TaskFrontmatter { name: "auth", depends: ["db"] } + body: "任务描述..."
fn parse_markdown(content: &str) -> Result<(TaskFrontmatter, String)>
```

### v2 完全复用

数据结构和解析逻辑可直接复用，只需调整结构体名称。

---

## 3. 任务名验证

### 关键文件

| 文件 | 说明 |
|------|------|
| `/Users/yansir/code/tmp/worktree/src/models/task_parser.rs:60-109` | 完整验证逻辑 |

### 验证规则

任务名必须是合法 git 分支名后缀：
- 不能含空格、路径分隔符、git 特殊字符 (`~ ^ : ? * [ @ {`)
- 不能以 `-` 或 `.` 开头
- 不能以 `.lock` 结尾
- 不能含 `..`

### v2 完全复用

不需要任何修改。

---

## 4. 变量展开

### 关键文件

| 文件 | 说明 |
|------|------|
| `/Users/yansir/code/tmp/worktree/src/services/executor/context.rs:1-50` | ExecutionContext 结构体 |
| `/Users/yansir/code/tmp/worktree/src/services/executor/context.rs:114-173` | `expand()` 变量替换 |
| `/Users/yansir/code/tmp/worktree/src/services/executor/context.rs:175-213` | `to_env_vars()` 环境变量转换 |

### v2 大幅简化

v1 的 ExecutionContext 有 phase、exit_reason、step_outputs 等复杂字段。

v2 简化为：

```rust
struct Context {
    task: String,      // ${task}
    branch: String,    // ${branch} = "wf/{task}"
    worktree: String,  // ${worktree}
    window: String,    // ${window} = "{task}"
    session: String,   // ${session}
    repo_root: String, // ${repo_root}
    step: String,      // ${step} = 当前 step name
}
```

`expand()` 方法可简化复用。`to_env_vars()` 用于设置 `WT_TASK` 等环境变量，`wf done` 依赖 `WT_TASK` 来确定当前任务。

---

## 5. JSONC 解析

### 关键文件

| 文件 | 说明 |
|------|------|
| `/Users/yansir/code/tmp/worktree/src/models/config.rs:148-170` | `WtConfig::load()` / `from_str()` |

### 核心逻辑

```rust
// 使用 json_comments crate 剥离注释
let stripped = json_comments::StripComments::new(content.as_bytes());
let config: Config = serde_json::from_reader(stripped)?;
```

### v2 完全复用

依赖 `json_comments` + `serde_json` 的模式不变。

---

## 6. 状态存储

### 关键文件

| 文件 | 说明 |
|------|------|
| `/Users/yansir/code/tmp/worktree/src/models/status.rs` | StatusStore, TaskState 定义 |

### 原子写入模式

```rust
// 写入 tmp 文件 → rename 原子替换
let tmp_path = format!("{}.tmp", path);
fs::write(&tmp_path, content)?;
fs::rename(&tmp_path, path)?;
```

### v2 简化复用

数据结构大幅简化（去掉 phase, instance），但原子写入模式直接复用。

---

## 7. Shell 命令执行

### 关键文件

| 文件 | 说明 |
|------|------|
| `/Users/yansir/code/tmp/worktree/src/services/command.rs` | `CommandRunner` 封装 |

### 核心 API

```rust
CommandRunner::new("git").run(&["status"])?;          // 执行
CommandRunner::new("git").output(&["branch"])?;       // 获取输出
CommandRunner::new("git").success(&["diff", "--quiet"]); // 布尔检查
CommandRunner::new("git").current_dir("/path").run(&[...])?; // 指定工作目录
```

### v2 完全复用

不需要任何修改。

---

## 8. Git 操作

### 关键文件

| 文件 | 说明 |
|------|------|
| `/Users/yansir/code/tmp/worktree/src/services/git.rs:47-60` | `create_worktree()`, `remove_worktree()` |
| `/Users/yansir/code/tmp/worktree/src/services/git.rs:62-87` | `delete_branch()`, `get_repo_root()`, `branch_exists()` |
| `/Users/yansir/code/tmp/worktree/src/services/git.rs:80-87` | `branch_exists()` |

### v2 注意

v2 中资源创建由用户在 workflow step 中显式调用（`git branch`, `git worktree add`），但 wf 自身仍需要这些函数用于：
- `wf reset` 清理资源
- `get_repo_root()` 计算变量

---

## 9. Tmux 操作

### 关键文件

| 文件 | 说明 |
|------|------|
| `/Users/yansir/code/tmp/worktree/src/services/multiplexer/tmux.rs` | TmuxBackend |
| `/Users/yansir/code/tmp/worktree/src/services/multiplexer/mod.rs:53-95` | Multiplexer trait |

### v2 所需的 tmux 操作

| 操作 | 用途 |
|------|------|
| `send_keys(session, window, keys)` | `in_window` step 发送命令 |
| `send_keys(session, window, "C-c")` | `wf stop` 发送 Ctrl+C |
| `focus_window(session, window)` | `wf enter` 切换窗口 |

v2 可以去掉 Multiplexer trait（MVP 只支持 tmux），直接封装为简单函数。

---

## 10. TUI

### 关键文件

| 文件 | 说明 |
|------|------|
| `/Users/yansir/code/tmp/worktree/src/tui/app.rs` | App 状态、TaskDisplay |
| `/Users/yansir/code/tmp/worktree/src/tui/ui.rs` | 渲染逻辑 |

### v2 参考

TUI 的框架结构（ratatui + crossterm、事件循环、状态刷新）可参考，但渲染内容需要重新设计（去掉 phase，改为 step 进度）。

---

## 11. .gitignore 模板

### 关键文件

| 文件 | 说明 |
|------|------|
| `/Users/yansir/code/tmp/worktree/src/commands/init/templates.rs:7-16` | gitignore 条目 |

### v2 内容

```gitignore
# wf - Workflow Task Runner
.wf/*
!.wf/tasks/
!.wf/config.jsonc
!.wf/hooks/
!.wf/templates/
!.wf/verify.md
```

---

## 复用优先级

| 优先级 | 模块 | 原因 |
|--------|------|------|
| **高** | Agent 自验证 (Stop Hook) | 核心差异化功能，逻辑复杂，直接复用 |
| **高** | Shell 命令执行 (CommandRunner) | 基础设施，100% 复用 |
| **高** | YAML frontmatter 解析 | 格式不变，直接复用 |
| **高** | 任务名验证 | 逻辑不变，直接复用 |
| **中** | JSONC 解析 | 模式复用，数据结构重写 |
| **中** | 变量展开 | 简化后复用 |
| **中** | 状态存储 (原子写入) | 模式复用，数据结构简化 |
| **中** | Git 操作 | 部分函数复用 |
| **低** | Tmux 操作 | 简化为几个函数 |
| **低** | TUI | 框架参考，内容重写 |
