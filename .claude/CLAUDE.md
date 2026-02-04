# wf - Workflow Task Runner

一个可配置的任务工作流执行器，让多个 AI agent 在独立的 git worktree 中并行开发。

## 核心概念

- **Task**: 定义在 `.wf/tasks/*.md` 的任务文件
- **Step**: workflow 中的执行单元（普通命令/checkpoint/in_window）
- **Checkpoint**: 暂停点，等待 `wf next` 继续
- **in_window**: 在 tmux window 中执行，等待 `wf done/fail/block`

## 项目结构

```
src/
├── main.rs           # 入口
├── cli.rs            # clap CLI 定义
├── model/
│   ├── config.rs     # Config + JSONC 加载
│   ├── state.rs      # StatusStore + 原子写入
│   └── task.rs       # TaskDefinition + frontmatter 解析
├── cmd/
│   ├── common.rs     # Project 上下文
│   ├── init.rs       # wf init
│   ├── create.rs     # wf create
│   ├── start.rs      # wf start (执行引擎)
│   ├── status.rs     # wf status/list
│   ├── control.rs    # wf next/retry/back/skip/stop/reset
│   ├── agent.rs      # wf done/fail/block
│   ├── enter.rs      # wf enter
│   └── log.rs        # wf log
└── util/
    ├── git.rs        # Git 操作
    ├── shell.rs      # Shell 执行
    ├── tmux.rs       # Tmux 操作
    └── variable.rs   # 变量展开
```

## 开发命令

```bash
cargo build           # 构建
cargo run -- --help   # 查看帮助
cargo run -- init     # 初始化项目
cargo test            # 运行测试
```

## 相关文档

- `docs/README.md` - 文档索引
- `docs/cli.md` - CLI 命令参考
- `docs/config.md` - 配置文件参考
- `docs/execution.md` - 执行模型
- `docs/data-model.md` - 数据模型
