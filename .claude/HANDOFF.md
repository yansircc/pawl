# Session Handoff

## 本次 Session 完成的工作

### 项目初始化
- 使用 `cargo init` 创建 Rust 项目
- 配置依赖：clap, serde, serde_json, anyhow, chrono, json_comments

### 核心基础设施
- **Config 加载** (`model/config.rs`): JSONC 解析，支持注释
- **Status 存储** (`model/state.rs`): 原子读写 (tmp+rename)
- **变量展开** (`util/variable.rs`): `${task}`, `${branch}`, `${worktree}` 等
- **Shell 执行** (`util/shell.rs`): 命令执行封装
- **Git 工具** (`util/git.rs`): `get_repo_root()`, `validate_branch_name()`
- **Tmux 操作** (`util/tmux.rs`): session/window 管理

### 执行引擎 (`cmd/start.rs`)
- 顺序执行 workflow steps
- Checkpoint 处理（暂停等待 `wf next`）
- in_window 处理（发送到 tmux，等待 agent 标记）
- Hook 触发（fire-and-forget）

### 命令实现
| 命令 | 状态 | 文件 |
|------|------|------|
| `wf init` | ✅ | `cmd/init.rs` |
| `wf create` | ✅ | `cmd/create.rs` |
| `wf start` | ✅ | `cmd/start.rs` |
| `wf status/list` | ✅ | `cmd/status.rs` |
| `wf next` | ✅ | `cmd/control.rs` |
| `wf retry` | ✅ | `cmd/control.rs` |
| `wf back` | ✅ | `cmd/control.rs` |
| `wf skip` | ✅ | `cmd/control.rs` |
| `wf stop` | ✅ | `cmd/control.rs` |
| `wf reset` | ✅ | `cmd/control.rs` |
| `wf done` | ✅ | `cmd/agent.rs` |
| `wf fail` | ✅ | `cmd/agent.rs` |
| `wf block` | ✅ | `cmd/agent.rs` |
| `wf enter` | ✅ | `cmd/enter.rs` |
| `wf log` | ⚠️ 基础版 | `cmd/log.rs` |

## 待实现功能

### 高优先级
1. **详细日志记录** - 将 step 的 stdout/stderr 保存到 `.wf/logs/`
2. **任务索引支持** - `wf start 1` 按索引启动任务

### 中优先级
3. **TUI 界面** - 使用 ratatui 实现交互式状态查看
4. **`--json` 输出** - `wf status --json` 支持

### 低优先级
5. **文件锁** - 并发写入 status.json 的保护
6. **Agent 自验证 (Stop Hook)** - 参考 v1 的实现

## 已知问题

- `wf log` 只显示基本状态，未实现完整日志记录
- 未实现任务名的 1-based 索引解析

## 关键文件索引

| 功能 | 文件 |
|------|------|
| 执行引擎 | `src/cmd/start.rs` |
| 状态存储 | `src/model/state.rs` |
| 配置加载 | `src/model/config.rs` |
| CLI 定义 | `src/cli.rs` |
| 默认配置模板 | `src/cmd/init.rs` (DEFAULT_CONFIG) |
