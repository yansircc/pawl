---
name: claude-cli
description: Claude CLI 命令参考，帮助构建可执行的 claude 命令
---

# Claude CLI

## 基本用法

```bash
claude                           # 交互式 REPL
claude "query"                   # 带提示启动 REPL
claude -p "query"                # 非交互式，执行后退出
cat file | claude -p "query"     # 管道输入
claude -c                        # 继续最近对话
claude -r "session" "query"      # 恢复指定 session
```

## 核心 Flags

| Flag | 说明 |
|------|------|
| `-p, --print` | **非交互模式**（自动化必须） |
| `--output-format` | `text`(默认)/`json`/`stream-json` |
| `--model` | 模型：`sonnet`/`opus`/`haiku` 或完整名 |
| `--system-prompt` | 替换系统提示 |
| `--append-system-prompt` | 追加到系统提示（推荐） |
| `--tools` | 限制工具：`"Bash,Edit,Read"` 或 `""` 禁用全部 |
| `--allowedTools` | 允许无确认：`"Bash(git *)" "Read"` |
| `--disallowedTools` | 禁用特定工具 |
| `--mcp-config` | MCP 配置：文件路径或 JSON |
| `--strict-mcp-config` | 只用指定的 MCP 配置 |
| `--setting-sources` | 设置源：`user,project,local` 或 `""` |
| `--settings` | 额外设置文件 |
| `--permission-mode` | 权限模式：`plan` 等 |
| `--dangerously-skip-permissions` | 跳过所有权限 |
| `--max-budget-usd` | API 花费上限（仅 -p） |
| `--max-turns` | 轮次上限（仅 -p） |
| `--json-schema` | 验证输出 JSON 结构 |
| `--agents` | 自定义子代理 JSON |
| `--debug` | 调试：`"api,mcp"` |
| `--verbose` | 详细输出 |

## 常用模式

```bash
# 自动化执行
claude -p "task" --output-format json

# 自定义系统提示
claude --append-system-prompt "Always use TypeScript" "query"
claude -p --system-prompt-file ./prompt.txt "query"

# 工具控制
claude --tools "Bash,Read" "query"
claude --allowedTools "Bash(git log *)" "query"

# MCP 配置
claude --mcp-config ./mcp.json "query"
claude --mcp-config '{"mcpServers":{...}}' "query"

# 资源限制
claude -p --max-budget-usd 5.00 --max-turns 10 "query"

# 子代理
claude --agents '{"reviewer":{"description":"...","prompt":"...","tools":["Read"],"model":"sonnet"}}' "query"
```

## 完整示例

```bash
claude -p \
  --model haiku \
  --system-prompt "Reply concisely." \
  --setting-sources "" \
  --tools "" \
  --mcp-config '{"mcpServers":{"bash":{"command":"node","args":["./mcp.js"]}}}' \
  --output-format stream-json \
  --verbose \
  "Say hello"
```

## 要点

1. `-p` 是非交互执行的关键
2. `--append-system-prompt` 比 `--system-prompt` 安全（保留默认能力）
3. 优先级：CLI flags > --settings > 项目设置 > 用户设置
4. 启用 `--output-format stream-json` 必须配合 `--verbose`

## 与 Codex 对比

| 操作 | Claude | Codex |
|------|--------|-------|
| 非交互 | `-p` | `exec` |
| 跳过确认 | `--dangerously-skip-permissions` | `--yolo` |
| JSON 输出 | `--output-format json` | `--json` |
| 会话恢复 | `-r` / `-c` | `resume` |

文档: https://code.claude.com/docs/llms.txt
