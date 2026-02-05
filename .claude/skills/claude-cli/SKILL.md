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
| `--input-format` | 输入格式：`text`/`stream-json`（仅 -p） |
| `--model` | 模型：`sonnet`/`opus`/`haiku` 或完整名 |
| `--fallback-model` | 过载时回退模型（仅 -p） |
| `--system-prompt` | 替换系统提示 |
| `--append-system-prompt` | 追加到系统提示（推荐） |
| `--system-prompt-file` | 从文件加载系统提示（仅 -p） |
| `--append-system-prompt-file` | 从文件追加系统提示（仅 -p） |
| `--tools` | 限制工具：`"Bash,Edit,Read"` 或 `""` 禁用全部 |
| `--allowedTools` | 允许无确认：`"Bash(git *)" "Read"` |
| `--disallowedTools` | 禁用特定工具 |
| `--mcp-config` | MCP 配置：文件路径或 JSON |
| `--strict-mcp-config` | 只用指定的 MCP 配置 |
| `--setting-sources` | 设置源：`user,project,local` 或 `""` 禁用 |
| `--settings` | 额外设置文件 |
| `--permission-mode` | 权限模式：`plan` 等 |
| `--dangerously-skip-permissions` | 跳过所有权限 |
| `--permission-prompt-tool` | MCP 工具处理权限提示（仅 -p） |
| `--max-budget-usd` | API 花费上限（仅 -p） |
| `--max-turns` | 轮次上限（仅 -p） |
| `--json-schema` | 验证输出 JSON 结构（仅 -p） |
| `--agent` | 指定当前 session 使用的 agent |
| `--agents` | 自定义子代理 JSON |
| `--add-dir` | 添加额外工作目录 |
| `--chrome` / `--no-chrome` | 启用/禁用 Chrome 浏览器集成 |
| `--init` / `--init-only` | 运行初始化 hooks（后者运行后退出） |
| `--session-id` | 指定 session ID（必须是 UUID） |
| `--fork-session` | 恢复时创建新 session（配合 -r/-c） |
| `--from-pr` | 恢复关联到指定 PR 的 session |
| `--remote` | 在 claude.ai 创建 web session |
| `--teleport` | 将 web session 恢复到本地终端 |
| `--no-session-persistence` | 禁用会话持久化（仅 -p） |
| `--disable-slash-commands` | 禁用 slash commands |
| `--include-partial-messages` | 包含部分流事件（需 stream-json） |
| `--betas` | API beta headers（仅 API key 用户） |
| `--plugin-dir` | 加载插件目录（可重复） |
| `--debug` | 调试：`"api,mcp"` 或 `"!statsig,!file"` |
| `--verbose` | 详细输出 |

## 省 Token 模式

最小化 token 消耗（适合简单任务）：

```bash
claude -p \
  --setting-sources "" \
  --strict-mcp-config --mcp-config '{"mcpServers":{}}' \
  --disable-slash-commands \
  --tools "" \
  "query"
```

效果：76k tokens → 1.5k tokens（约 98% 减少）

## 结构化输出

```bash
# 布尔值
claude -p --output-format json \
  --json-schema '{"type":"object","properties":{"result":{"type":"boolean"}},"required":["result"]}' \
  "query" | jq '.structured_output'

# 复杂结构
claude -p --output-format json \
  --json-schema '{"type":"object","properties":{"items":{"type":"array","items":{"type":"string"}}},"required":["items"]}' \
  "query"
```

## 子代理格式

```bash
claude --agents '{
  "reviewer": {
    "description": "代码审查专家，代码变更后主动使用",
    "prompt": "你是资深代码审查员，关注质量、安全和最佳实践",
    "tools": ["Read", "Grep", "Glob"],
    "model": "sonnet"
  }
}'
```

| 字段 | 必须 | 说明 |
|------|------|------|
| `description` | 是 | 何时调用的描述 |
| `prompt` | 是 | 子代理的系统提示 |
| `tools` | 否 | 可用工具，默认继承全部 |
| `model` | 否 | `sonnet`/`opus`/`haiku`/`inherit` |

## System Prompt 策略

| Flag | 行为 | 使用场景 |
|------|------|----------|
| `--system-prompt` | 完全替换 | 需要完全控制 |
| `--append-system-prompt` | 追加 | 保留默认能力（推荐） |
| `--system-prompt-file` | 从文件替换 | 版本控制提示 |
| `--append-system-prompt-file` | 从文件追加 | 版本控制追加 |

## 与 Codex 对比

| 操作 | Claude | Codex |
|------|--------|-------|
| 非交互 | `-p` | `exec` |
| 跳过确认 | `--dangerously-skip-permissions` | `--yolo` |
| JSON 输出 | `--output-format json` | `--json` |
| 会话恢复 | `-r` / `-c` | `resume` |

文档: https://code.claude.com/docs/llms.txt
