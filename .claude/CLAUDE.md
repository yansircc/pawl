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
│   ├── state.rs      # StatusStore + 原子写入 + 文件锁
│   └── task.rs       # TaskDefinition + frontmatter 解析
├── cmd/
│   ├── common.rs     # Project 上下文 + log_dir/log_path/prev_log_path
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
    ├── tmux.rs       # Tmux 操作
    └── variable.rs   # 变量展开（含日志路径变量）
```

## 日志系统

### 日志格式

in_window 步骤完成后生成 JSON 元数据日志：

```
.wf/logs/{task}/step-{N}-{slug}.json
```

```json
{
  "step": 1,
  "name": "Develop",
  "type": "in_window",
  "command": "claude -p ...",
  "completed": "2026-02-05T12:38:21+00:00",
  "exit_code": 0,
  "status": "success"
}
```

### 日志相关变量

| 变量 | 环境变量 | 说明 |
|------|----------|------|
| `${log_dir}` | `WF_LOG_DIR` | 任务日志目录 |
| `${log_path}` | `WF_LOG_PATH` | 当前 step 日志路径 |
| `${prev_log}` | `WF_PREV_LOG` | 前一个 step 日志路径 |
| `${step_index}` | `WF_STEP_INDEX` | 当前 step 索引（0-based）|

### 读取 Claude 输出

Claude CLI 的实际输出在其 transcript 中：
```bash
# 使用 --output-format=stream-json 时，最后一行是 result
grep '"type":"result"' output.log | jq -r '.result'

# 或直接读取 Claude 的 transcript
cat ~/.claude/projects/{hash}/{session-id}.jsonl
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
