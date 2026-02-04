# Product Requirements Document

## Problem

开发者使用 AI agent（如 Claude）辅助编码时，存在以下痛点：

1. **串行瓶颈** — 一次只能让一个 agent 处理一个任务
2. **环境管理繁琐** — 每次要手动创建 branch、worktree、tmux window
3. **状态不可见** — 多个 agent 并行时，不知道哪个任务到哪一步了
4. **缺乏控制** — 没有检查点，agent 跑完了不知道质量如何
5. **清理遗漏** — 任务完成后忘记删除 worktree、branch

## Solution

wf 是一个 CLI 工具，通过可配置的 workflow：

1. 自动化环境创建/清理（branch, worktree, tmux window）
2. 在独立 worktree 中并行运行多个 agent
3. 提供 TUI 查看所有任务状态
4. 在关键节点设置 checkpoint，暂停等待人工确认
5. 记录每个 step 的执行日志

## Target User

使用 AI coding agent 的开发者，日常开发中有多个并行任务需求。

## Core User Flow

```
1. 定义任务  →  .wf/tasks/auth.md
2. 启动任务  →  wf start auth
3. 观察进度  →  wf status
4. 人工介入  →  wf enter auth / wf log auth
5. 继续推进  →  wf next auth
6. 任务完成  →  自动清理
```

## Functional Requirements

### FR-1: 项目初始化

```bash
wf init
```

- 创建 `.wf/` 目录结构
- 生成默认 `config.jsonc`
- 添加 `.gitignore` 条目

### FR-2: 任务定义

- 任务以 markdown 文件定义在 `.wf/tasks/` 目录
- YAML frontmatter 包含 `name` 和可选的 `depends`
- Markdown body 是任务描述

### FR-3: Workflow 配置

- 在 `config.jsonc` 中定义 workflow（Step 数组）
- 所有任务共享同一个 workflow
- Step 支持变量展开（`${task}`, `${branch}` 等）

### FR-4: 任务执行

- `wf start <task>` 从 step 0 开始执行 workflow
- 普通 step 同步执行，等待 exit code
- `in_window` step 发送到 tmux window，等待 `wf done`
- Checkpoint 暂停等待 `wf next`

### FR-5: 状态查看

- `wf status` 显示所有任务的当前状态
- 包含：任务名、当前 step、状态、耗时
- 支持 TUI 交互模式和 JSON 输出

### FR-6: 流程控制

- `wf next <task>` — 跳过 checkpoint，继续执行
- `wf retry <task>` — 重新执行当前失败的 step
- `wf back <task>` — 回退到上一个 step
- `wf skip <task>` — 跳过当前 step
- `wf stop <task>` — 停止当前进程
- `wf reset <task>` — 重置到 step 0，清理资源

### FR-7: 日志查看

- 每个 step 的 stdout/stderr 保存到 log 文件
- `wf log <task>` 查看当前 step 的日志
- `wf log <task> --step N` 查看指定 step 的日志

### FR-8: Agent 状态标记

在 tmux window 中执行的 agent 通过以下命令标记状态：

- `wf done` — 标记当前 step 成功
- `wf fail [reason]` — 标记失败
- `wf block [reason]` — 标记需要人工介入

### FR-9: Hook

- 在 config 中定义事件触发的 shell 命令
- 支持 `task.completed`, `step.failed` 等事件

### FR-10: 依赖管理

- 任务可以声明依赖其他任务
- `wf start` 检查依赖是否满足
- `wf status` 显示被阻塞的任务及原因

### FR-11: 窗口管理

- `wf enter <task>` 切换到任务的 tmux window
- `wf stop <task>` 向 window 发送 Ctrl+C

## Non-Functional Requirements

### NFR-1: 性能
- `wf status` 响应时间 < 100ms
- 不依赖后台 daemon

### NFR-2: 可靠性
- 状态存储为 JSON 文件，可手动编辑恢复
- step 执行失败不影响其他任务
- 优雅处理 tmux session 不存在的情况

### NFR-3: 简洁性
- 核心代码 < 1500 行
- 概念少于 5 个（Step, Checkpoint, Hook, Task, Workflow）
- config.jsonc 易读，不需要文档就能理解

## Out of Scope (MVP)

- Zellij 支持（只支持 tmux）
- Git 快照/回退（checkpoint snapshot）
- Web UI
- 远程执行
- 任务模板
- 并发控制（同时运行的最大任务数）
