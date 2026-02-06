# Session Handoff

## 本次 Session 完成的工作

### Event Sourcing 迁移

将持久化从双源（`status.json` + JSONL 日志）迁移到单源 Event Sourcing 架构。JSONL 事件日志成为唯一的 source of truth，`status.json` 彻底移除。

**新增文件**:
- `src/model/event.rs` — Event 枚举（12 种事件）、AgentResult、`replay()` 函数、13 个单元测试

**删除文件**:
- `src/model/log.rs` — StepLog 被 Event 完全取代

**重写文件**:

| 文件 | 改动 |
|------|------|
| `src/model/state.rs` | 移除 StatusStore 及所有 mutation helpers，仅保留 TaskState/TaskStatus/StepStatus 纯数据结构 |
| `src/model/mod.rs` | 更新 exports：移除 log/StatusStore，添加 event/Event/AgentResult |
| `src/cmd/common.rs` | 移除 `status` 字段/`save_status()`/`append_log()`/`read_logs()`；新增 `append_event()`（带 fs2 文件锁）/`read_events()`/`replay_task()` |
| `src/cmd/start.rs` | 所有 status 写入 + log append → `append_event()`；`&mut Project` → `&Project` |
| `src/cmd/control.rs` | next/retry/back/skip/stop/reset/on_exit 全部改为 event append |
| `src/cmd/agent.rs` | done/fail/block 改为 `AgentReported` event |
| `src/cmd/status.rs` | `project.status.get()` → `project.replay_task()` |
| `src/cmd/log.rs` | 读 Event 替代 StepLog |
| `src/cmd/capture.rs` | 替换 status.get() |
| `src/cmd/wait.rs` | 轮询改为读 JSONL + replay |
| `src/tui/data/live.rs` | 移除 StatusStore::load()，改为逐任务 replay |

**关键设计决策**:
- `replay()` 中 OnExit 事件仅当 step 尚未被 AgentReported 处理时才生效（防竞争）
- TaskReset 不删文件，只 append 事件（replay 遇到后清空状态）
- 所有 cmd 函数改为 `&Project`（不可变借用），消除借用检查器冲突
- `append_event()` 使用 `fs2::lock_exclusive()` + 错误传播 `?`

---

## 之前 Session 完成的功能

| 功能 | 状态 |
|------|------|
| 核心执行引擎 + E2E 测试 | ✅ |
| Event Sourcing（JSONL 单源） | ✅ |
| Session ID 提取 + Transcript | ✅ |
| 变量展开（11 个变量） | ✅ |
| Stop Hook | ✅ |
| TUI 界面 | ✅ |
| `wf init` 引导式初始化 | ✅ |

---

## 关键文件索引

| 功能 | 文件 |
|------|------|
| CLI 定义 | `src/cli.rs` |
| 配置模型 | `src/model/config.rs` |
| 事件模型 + replay | `src/model/event.rs` |
| 状态投影类型 | `src/model/state.rs` |
| 执行引擎 | `src/cmd/start.rs` |
| Agent 命令 | `src/cmd/agent.rs` |
| 控制命令 | `src/cmd/control.rs` |
| 公共工具（事件读写） | `src/cmd/common.rs` |
| 变量展开 | `src/util/variable.rs` |
| tmux 操作 | `src/util/tmux.rs` |
| TUI 数据层 | `src/tui/data/live.rs` |

---

## 相关文档

- `.claude/CLAUDE.md` - 项目概述（已更新）
- `.claude/insights.md` - 深度分析（已更新，标记已解决项）
- `docs/config.md` - 配置文件参考
- `docs/log-system.md` - 日志系统设计（内容可能需要更新以反映 Event Sourcing）
