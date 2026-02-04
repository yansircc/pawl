# wf v2 Spec

## Vision

wf 是一个可配置的任务工作流执行器。它让多个 AI agent 在独立的 git worktree 中并行开发，同时保持人类的控制权。

## Core Insight

一切皆 shell 命令。

- 创建 branch → `git branch wf/${task}`
- 启动 agent → `claude '@task.md'`
- 通知 → `curl webhook`
- 验证 → `bun typecheck`

没有特殊的 "agent" 概念、"resource" 概念、"notification" 概念。所有东西都是一条 shell 命令，由 wf 按顺序执行。

## Documents

| 文件 | 内容 |
|------|------|
| [concepts.md](./concepts.md) | 核心概念：Step, Checkpoint, Hook |
| [prd.md](./prd.md) | 产品需求文档 |
| [user-stories.md](./user-stories.md) | 用户故事和场景 |
| [config.md](./config.md) | 配置文件参考 |
| [cli.md](./cli.md) | CLI 命令参考 |
| [execution.md](./execution.md) | 执行模型 |
| [data-model.md](./data-model.md) | 数据模型和状态存储 |
| [reusable-code.md](./reusable-code.md) | v1 可复用代码索引（含绝对路径） |

## v1 项目位置

```
/Users/yansir/code/tmp/worktree
```

关键可复用模块（按优先级）：

1. **Agent 自验证 (Stop Hook)** — `src/commands/init/templates.rs:68-173`
2. **Shell 命令执行** — `src/services/command.rs`
3. **YAML frontmatter 解析** — `src/models/task_parser.rs`
4. **任务名验证** — `src/models/task_parser.rs:60-109`
5. **变量展开** — `src/services/executor/context.rs:114-173`
