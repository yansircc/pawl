# wf - Workflow Task Runner

一个可配置的任务工作流执行器，让多个 AI agent 在独立的 git worktree 中并行开发。

## 核心概念

- **Task**: 定义在 `.wf/tasks/*.md` 的任务文件
- **Step**: workflow 中的执行单元（普通命令/checkpoint/in_window）
- **Checkpoint**: 暂停点，等待 `wf next` 继续
- **in_window**: 在 tmux window 中执行，等待 `wf done/fail/block`
- **Stop Hook**: `wf done` 前的验证命令，验证失败则拒绝标记完成

## 项目结构

```
src/
├── main.rs           # 入口
├── cli.rs            # clap CLI 定义
├── model/
│   ├── config.rs     # Config + JSONC 加载 + Step.stop_hook
│   ├── log.rs        # StepLog 枚举（JSONL 日志条目）
│   ├── state.rs      # StatusStore + 原子写入 + 文件锁
│   └── task.rs       # TaskDefinition + frontmatter 解析
├── cmd/
│   ├── common.rs     # Project 上下文 + log_file/task_file/append_log/read_logs
│   ├── init.rs       # wf init
│   ├── create.rs     # wf create
│   ├── start.rs      # wf start (执行引擎)
│   ├── status.rs     # wf status/list (支持 --json)
│   ├── control.rs    # wf next/retry/back/skip/stop/reset + _on-exit
│   ├── agent.rs      # wf done/fail/block (含 stop_hook 验证)
│   ├── capture.rs    # wf capture (tmux 内容捕获)
│   ├── wait.rs       # wf wait (等待状态变化)
│   ├── enter.rs      # wf enter
│   └── log.rs        # wf log (支持 --step/--all)
├── tui/              # 交互式 TUI 界面
│   ├── app.rs        # 主循环
│   ├── state/        # 状态层
│   ├── view/         # 渲染层
│   ├── event/        # 事件处理
│   └── data/         # 数据层
└── util/
    ├── git.rs        # Git 操作
    ├── shell.rs      # Shell 执行
    ├── tmux.rs       # Tmux 操作 + session_id 提取
    └── variable.rs   # 变量展开
```

## 日志系统

### 日志格式

每个任务一个 JSONL 文件：`.wf/logs/{task}.jsonl`

每个 step 完成后追加一行 JSON：

**普通命令 step:**
```json
{"type":"command","step":0,"exit_code":0,"duration":5.2,"stdout":"...","stderr":""}
```

**in_window step:**
```json
{"type":"in_window","step":1,"session_id":"xxx","transcript":"/path/to/xxx.jsonl","status":"success"}
```

**checkpoint step:**
```json
{"type":"checkpoint","step":2}
```

### 日志相关变量

| 变量 | 环境变量 | 说明 |
|------|----------|------|
| `${log_file}` | `WF_LOG_FILE` | 任务日志文件路径 (.jsonl) |
| `${task_file}` | `WF_TASK_FILE` | 任务定义文件路径 (.md) |
| `${step_index}` | `WF_STEP_INDEX` | 当前 step 索引（0-based）|
| `${base_branch}` | `WF_BASE_BRANCH` | 基础分支（创建任务分支的起点）|

### 读取日志

```bash
# 查看任务所有日志
wf log <task> --all

# 查看特定 step 日志
wf log <task> --step 2

# 使用 jq 读取 JSONL
jq -s '.[] | select(.step==1)' .wf/logs/task.jsonl

# 获取最后一个 step 的 transcript
tail -1 .wf/logs/task.jsonl | jq -r '.transcript'
```

## CLI 命令

| 命令 | 说明 |
|------|------|
| `wf init` | 初始化项目 |
| `wf create <name>` | 创建任务 |
| `wf list` | 列出所有任务 |
| `wf start <task>` | 启动任务 |
| `wf status [task] [--json]` | 查看状态 |
| `wf next <task>` | 通过 checkpoint |
| `wf retry <task>` | 重试当前步骤 |
| `wf back <task>` | 回退一步 |
| `wf skip <task>` | 跳过当前步骤 |
| `wf stop <task>` | 停止任务 |
| `wf reset <task>` | 重置任务 |
| `wf done <task>` | 标记完成 (agent) |
| `wf fail <task>` | 标记失败 (agent) |
| `wf block <task>` | 标记阻塞 (agent) |
| `wf enter <task>` | 进入 tmux window |
| `wf capture <task> [--json]` | 捕获 tmux 内容 |
| `wf wait <task> --until <status>` | 等待状态 |
| `wf log <task> [--step N] [--all]` | 查看日志 |
| `wf tui` | 打开交互式 TUI 界面 |

## 开发命令

```bash
cargo build           # 构建
cargo install --path . # 安装到 ~/.cargo/bin
cargo test            # 运行测试
```

## 相关文档

- `docs/README.md` - 文档索引
- `docs/cli.md` - CLI 命令参考
- `docs/config.md` - 配置文件参考
- `docs/execution.md` - 执行模型
- `docs/data-model.md` - 数据模型
- `docs/log-system.md` - 日志系统设计
