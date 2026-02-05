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
│   ├── common.rs     # Project 上下文 + slugify/log_dir/log_path
│   ├── init.rs       # wf init
│   ├── create.rs     # wf create
│   ├── start.rs      # wf start (执行引擎 + 日志记录)
│   ├── status.rs     # wf status/list (支持 --json)
│   ├── control.rs    # wf next/retry/back/skip/stop/reset + _on-exit
│   ├── agent.rs      # wf done/fail/block (含 stop_hook 验证)
│   ├── capture.rs    # wf capture (tmux 内容捕获)
│   ├── wait.rs       # wf wait (等待状态变化)
│   ├── enter.rs      # wf enter
│   └── log.rs        # wf log (支持 --step/--all)
├── tui/              # 交互式 TUI 界面
│   ├── app.rs        # 主循环 (事件处理 + 渲染)
│   ├── state/        # 状态层 (可单元测试)
│   │   ├── app_state.rs    # 根状态
│   │   ├── task_list.rs    # 任务列表状态
│   │   ├── task_detail.rs  # 任务详情状态
│   │   ├── tmux_view.rs    # Tmux 视图状态
│   │   └── reducer.rs      # 纯状态转换函数
│   ├── view/         # 渲染层
│   │   ├── layout.rs       # 主布局
│   │   ├── task_list.rs    # 任务列表组件
│   │   ├── task_detail.rs  # 任务详情组件
│   │   ├── tmux_pane.rs    # Tmux 内容组件
│   │   ├── status_bar.rs   # 状态栏
│   │   ├── help_popup.rs   # 帮助弹窗
│   │   └── style.rs        # 样式定义
│   ├── event/        # 事件处理
│   │   ├── action.rs       # Action 枚举
│   │   └── input.rs        # 按键处理
│   └── data/         # 数据层
│       ├── provider.rs     # DataProvider trait
│       └── live.rs         # 实际实现
└── util/
    ├── git.rs        # Git 操作
    ├── shell.rs      # Shell 执行
    ├── tmux.rs       # Tmux 操作 + capture_pane
    └── variable.rs   # 变量展开
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
