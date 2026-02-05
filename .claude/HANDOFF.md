# Session Handoff

## 本次 Session 完成的工作

### Log Pipeline 简化重构

**目标**：实现稳定的日志管道，让后续 step 可以读取前置 step 的输出

**最终方案**：激进简化 - 只记录元数据，让 Judge 自己读取 Claude 的 transcript

#### 完成的改动

| 改动 | 说明 |
|------|------|
| 移除 pipe-pane | 不再流式捕获终端输出 |
| 移除 ANSI 过滤 | 删除 text.rs 和 regex 依赖 |
| JSON 元数据日志 | `.wf/logs/{task}/step-{n}-{slug}.json` |
| 新增变量 | `${log_dir}`, `${log_path}`, `${prev_log}`, `${step_index}` |
| 修复 worktree bug | `_on-exit` 回到 repo_root 执行 |

#### 日志格式

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

#### 设计决策

**为什么只记录元数据？**
- Claude CLI 自己维护 transcript（`~/.claude/projects/*/session.jsonl`）
- `--output-format=stream-json` 的最后一行包含完整 result
- Judge 可以用 shell 命令提取需要的信息
- 避免复杂的终端输出处理（ANSI codes、断行等）

---

## 待实现功能

### Session 软链接（用户建议）

在日志目录创建软链接指向 Claude 的 transcript，简化 Judge 读取：

```
.wf/logs/{task}/
  step-1-xxx.json           # 元数据
  latest-session -> ~/.claude/projects/{hash}/{session-id}.jsonl
```

这样 Judge 可以直接：
```bash
cat ${log_dir}/latest-session | jq '.result'
```

**待解决**：
- 如何获取 session_id（从 Claude JSON 输出解析）
- 如何定位 Claude projects 目录

---

## 功能完成状态

| 功能 | 状态 |
|------|------|
| 核心执行引擎 | ✅ |
| 日志记录（普通 step） | ✅ |
| 日志记录（in_window）| ✅ JSON 元数据 |
| 变量展开（含日志路径）| ✅ |
| Stop Hook | ✅ |
| TUI 界面 | ✅ |
| Claude/Codex CLI Skills | ✅ |
| Session 软链接 | ❌ 待实现 |

---

## 关键文件索引

| 功能 | 文件 |
|------|------|
| CLI 定义 | `src/cli.rs` |
| 执行引擎 | `src/cmd/start.rs` |
| Agent 命令 | `src/cmd/agent.rs` |
| 控制命令 | `src/cmd/control.rs` |
| 公共工具 | `src/cmd/common.rs` |
| 变量展开 | `src/util/variable.rs` |
| tmux 操作 | `src/util/tmux.rs` |
| 状态存储 | `src/model/state.rs` |

---

## 测试验证

在 `/Users/yansir/code/nextjs-project/try-wt` 测试通过：
- `wf start` → `wf status` → 日志生成正确
- JSON 元数据格式正确
- `_on-exit` 在 repo_root 正确执行
