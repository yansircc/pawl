---
name: codex-cli
description: OpenAI Codex CLI 命令参考，帮助构建可执行的 codex 命令
---

# Codex CLI

安装: `npm i -g @openai/codex` 或 `brew install --cask codex`

## 基本用法

```bash
codex                            # 交互式 UI
codex "query"                    # 带提示启动
codex exec "query"               # 非交互式（自动化必须）
codex resume --last              # 恢复上一会话
codex resume <id>                # 恢复指定会话
codex fork <id> "query"          # 从会话分支
```

## 子命令

| 命令 | 说明 |
|------|------|
| `exec` | 非交互执行，支持 `--json` |
| `resume` | 恢复会话（`--last`/`--all`/`<id>`） |
| `fork` | 分支会话 |
| `apply` | 应用云任务 diff |
| `login/logout` | 认证管理 |
| `cloud` | 云任务（experimental） |
| `mcp` | MCP 服务器模式（experimental） |

## 核心 Flags

| Flag | 说明 |
|------|------|
| `--model` | 模型：`gpt-5-codex`(默认)/`o3` 等 |
| `--cd` | 工作目录 |
| `--add-dir` | 额外目录写权限 |
| `--sandbox` | `read-only`/`workspace-write`/`danger-full-access` |
| `--ask-for-approval` | `untrusted`/`on-failure`/`on-request`/`never` |
| `--yolo` | 跳过审批和沙箱（`--dangerously-bypass-approvals-and-sandbox`） |
| `--full-auto` | 低摩擦本地模式 |
| `-c key=value` | 覆盖配置 |
| `--profile` | 配置 profile |
| `-i, --image` | 附加图片 |
| `--search` | 启用 web 搜索 |
| `--oss` | 本地模型（需 Ollama） |

## exec 专用

| Flag | 说明 |
|------|------|
| `--json` | JSON 事件输出 |
| `--output-last-message <file>` | 保存最终响应 |
| `--skip-git-repo-check` | 允许非 Git 仓库 |

## 审批模式

- **auto**（默认）：工作目录内自由操作，目录外需确认
- **read-only**：只读，修改需确认
- **full-access**：无限制

交互式切换：`/approvals`

## 常用模式

```bash
# 自动化执行
codex exec "task" --json

# 工作目录和权限
codex --cd /project --add-dir ../lib "query"
codex --sandbox workspace-write "query"

# 审批控制
codex --ask-for-approval never "query"
codex --yolo "query"
codex --full-auto "query"

# 配置覆盖
codex -c model=o3 -c sandbox=workspace-write "query"

# 图片输入
codex -i screenshot.png "describe this"
```

## 完整示例

```bash
codex exec \
  --model gpt-5-codex \
  --sandbox workspace-write \
  --ask-for-approval never \
  --json \
  "implement HTTP server"
```

## 交互式命令

`/model` `/approvals` `/review` `/fork` `Ctrl+G`(编辑器)

## 要点

1. `exec` 是非交互执行的关键
2. 配置优先级：CLI flags > 环境变量 > `~/.codex/config.toml`
3. 生产环境推荐 `workspace-write` 或 `read-only` 沙箱

文档: https://developers.openai.com/codex/cli/
