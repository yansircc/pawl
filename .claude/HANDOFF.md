# Session Handoff

## 本次 Session 完成的工作

### 日志系统 JSONL 重构

**目标**：将日志系统从"每个 step 一个文件"改为"每个 task 一个 JSONL 文件"

#### 完成的改动

| 改动 | 说明 |
|------|------|
| 新建 `src/model/log.rs` | `StepLog` 枚举（Command/InWindow/Checkpoint） |
| 新增 session_id 提取 | `tmux.rs` 添加 `extract_session_id()` 和 `get_transcript_path()` |
| 变量系统重构 | 删除 `log_dir/log_path/prev_log`，新增 `log_file/task_file` |
| Project 日志方法 | 新增 `log_file()/task_file()/append_log()/read_logs()` |
| 普通 step 日志 | 使用 JSONL 追加，记录 stdout/stderr |
| in_window 日志 | 提取 session_id 和 transcript 路径 |
| wf log 命令重写 | 读取 JSONL 并格式化输出 |

#### 新日志格式

**文件路径**: `.wf/logs/{task}.jsonl`

```jsonl
{"type":"command","step":0,"exit_code":0,"duration":5.2,"stdout":"...","stderr":""}
{"type":"in_window","step":1,"session_id":"xxx","transcript":"/path/to/xxx.jsonl","status":"success"}
{"type":"checkpoint","step":2}
```

#### 新变量

| 变量 | 环境变量 | 说明 |
|------|----------|------|
| `${log_file}` | `WF_LOG_FILE` | 任务日志文件路径 |
| `${task_file}` | `WF_TASK_FILE` | 任务定义文件路径 |

#### 已删除

- `${log_dir}` / `WF_LOG_DIR`
- `${log_path}` / `WF_LOG_PATH`
- `${prev_log}` / `WF_PREV_LOG`
- `slugify()` 函数
- 每个 step 单独的日志文件

---

## 功能完成状态

| 功能 | 状态 |
|------|------|
| 核心执行引擎 | ✅ |
| 日志记录（JSONL） | ✅ |
| Session ID 提取 | ✅ |
| Transcript 路径解析 | ✅ |
| 变量展开 | ✅ |
| Stop Hook | ✅ |
| TUI 界面 | ✅ |

---

## 关键文件索引

| 功能 | 文件 |
|------|------|
| CLI 定义 | `src/cli.rs` |
| 日志数据结构 | `src/model/log.rs` |
| 执行引擎 | `src/cmd/start.rs` |
| Agent 命令 | `src/cmd/agent.rs` |
| 控制命令 | `src/cmd/control.rs` |
| 公共工具 | `src/cmd/common.rs` |
| 日志查看 | `src/cmd/log.rs` |
| 变量展开 | `src/util/variable.rs` |
| tmux 操作 | `src/util/tmux.rs` |
| 状态存储 | `src/model/state.rs` |

---

## 相关文档

- `docs/log-system.md` - 日志系统设计文档
- `.claude/CLAUDE.md` - 项目概述（已更新）
