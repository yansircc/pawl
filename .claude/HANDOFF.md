# Session Handoff

## 本次 Session 完成的工作

### 1. Claude/Codex CLI Skills

新增两个 skill 文档，帮助 agent 构建可执行的 CLI 命令：

| Skill | 位置 | 用途 |
|-------|------|------|
| `claude-cli` | `.claude/skills/claude-cli/SKILL.md` | Claude Code CLI 参考 |
| `codex-cli` | `.claude/skills/codex-cli/SKILL.md` | OpenAI Codex CLI 参考 |

### 2. Agent 自验证机制调研

**核心发现**：

| CLI | Stop Hook | 自验证方式 |
|-----|-----------|-----------|
| Claude | ✅ 完整支持 `decision: block` | Stop Hook 强制 |
| Codex | ❌ 无（PR #9796 被拒绝） | 只能靠 prompt + output |

**决策**：放弃 Stop Hook 方式，采用更通用的 **output → judge → mark** 流程：
1. Agent 执行任务，输出保存到日志
2. 另一个 agent (如 haiku) 读取日志 + 原始任务，判断完成情况
3. 根据判断结果执行 `wf done/fail/block`

### 3. Log Pipeline 现状分析

**问题发现**：

| 步骤类型 | 日志记录 | 问题 |
|----------|----------|------|
| 普通 step | ✅ `.wf/logs/{task}/step-{n}-{slug}.log` | 正常 |
| in_window step | ❌ **没有记录** | 无法传递给下一步 |

**缺失功能**：
- in_window 步骤没有输出捕获
- 没有 step 间的输出传递机制（如 `${prev_output}`）
- `wf capture` 只能获取 tmux 可见区域，不是完整日志

---

## 待实现功能

### Log Pipeline（高优先级）

需要实现稳定可靠的日志管道，让执行结果可以往下传递：

1. **in_window 输出捕获**
   - 方案 A: `script` 命令记录
   - 方案 B: `tmux pipe-pane` 持续写入
   - 方案 C: 要求 agent 用 `-p --output-format json > log`

2. **变量传递**
   - 新增 `${prev_output}` 或 `${step_N_output}` 变量
   - 或用文件路径 `${log_dir}/step-{N}-{name}.log`

3. **Judge Step 类型**
   ```jsonc
   {
     "name": "Judge",
     "run": "claude -p --model haiku '读取 ${prev_output}，判断任务是否完成'",
     "judge": true  // 特殊标记，根据输出执行 wf done/fail/block
   }
   ```

---

## 功能完成状态

| 功能 | 状态 |
|------|------|
| 核心执行引擎 | ✅ |
| 日志记录（普通 step） | ✅ |
| 日志记录（in_window） | ❌ 待实现 |
| Log Pipeline | ❌ 待实现 |
| 任务索引 | ✅ |
| JSON 输出 | ✅ |
| 文件锁 | ✅ |
| Stop Hook | ✅ |
| tmux 捕获 | ✅ |
| TUI 界面 | ✅ |
| 所有 Hooks | ✅ |
| Claude/Codex CLI Skills | ✅ |

---

## 关键文件索引

| 功能 | 文件 |
|------|------|
| CLI 定义 | `src/cli.rs` |
| 执行引擎 | `src/cmd/start.rs` |
| Agent 命令 | `src/cmd/agent.rs` |
| 公共工具 | `src/cmd/common.rs` |
| 日志显示 | `src/cmd/log.rs` |
| tmux 捕获 | `src/cmd/capture.rs` |
| 状态存储 | `src/model/state.rs` |
| TUI | `src/tui/*.rs` |

---

## 实验记录

### Stop Hook 自验证实验

```bash
# 测试命令
ccc -p --model haiku --max-turns 10 \
  --settings /tmp/test-hook/settings.json \
  --output-format json \
  "Say hello and finish"

# 结果：4 轮完成，$0.004885
# 机制可行，但决定放弃 Stop Hook 改用更通用方案
```

### tmux 输出捕获实验

| 方法 | 可行性 |
|------|--------|
| `tmux capture-pane -p` | ✅ 但只有可见区域 |
| `script` 命令 | ✅ 完整记录 |
| tee 到文件 | ✅ 可行 |
| Claude transcript | ✅ `~/.claude/projects/*/session.jsonl` |

---

## 下一步

**进入 Plan Mode 实现 Log Pipeline**
