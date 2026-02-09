# Session Handoff

## Current Session (S44): Agent Driver 概念 + orchestrate.md 重构

### What changed

**1. orchestrate.md Recipe 重构**

解决 S42 压测发现的 5/5 agent 锚定到 Git Worktree 的问题。重排 recipe 顺序，新增 Plain Workflow 和 Agent Driver：

- **Plain Workflow**（新增，排第一）：最简 config，无 vars/git/isolation
- **Work Steps: 2 Dimensions**：解耦 worktree 依赖（删除 `cd ${worktree}` 前提）
- **Agent Driver**（替换 AI Worker Pattern）：4 操作框架 start/send/stop/read
- **Retry Feedback Loop**：改用 `PAWL_RETRY_COUNT` 检测重试（比 `PAWL_LAST_VERIFY_OUTPUT` 非空检查更可靠）
- **Git Worktree Skeleton**：从第一个 recipe 移到倒数第二
- **Claude Code Driver**（替换 Worker with Session Resume）：引用 claude-driver.sh + 省 token 提示

**2. Agent Driver 模板 (claude-driver.sh)**

`pawl init` 现在在 `.pawl/skills/pawl/references/claude-driver.sh` 生成开箱即用的 Claude Code driver：

- stdin 传 prompt（`cat $PAWL_TASK_FILE | driver start`）
- `PAWL_RUN_ID` 作为 `--session-id`（首次）或 `-r`（retry resume）
- `PAWL_RETRY_COUNT` 检测重试（比检查 verify_output 是否非空更可靠）
- settings.json Stop hook 触发 `pawl done`（上行路径）
- init.rs 嵌入模板，自动 chmod +x

**3. 实测验证**（sonnet agent，/tmp/pawl-driver-test）

| 场景 | 结果 |
|------|------|
| 首次运行：stdin → agent → verify 通过 | ✓ 6.8s |
| Retry：verify 失败 → `-r` 恢复 session → 修正 → 通过 | ✓ 22.3s |
| PAWL_RUN_ID 作为 session-id | ✓ |
| `pawl init` 生成 driver（可执行） | ✓ |

### Key Design Insights (S43-S44 贯穿讨论)

**Agent Driver 本质**

从 S43 的 foreman-worker 模型推导到 S44 的 essence 压缩：

```
Driver 的不可约内核:
  down: stdin → agent launch (retry 分支 + session 管理)
  up:   settings.json → Stop hook → pawl done
```

关键推导路径：
1. Agent = process + judgment + protocol participation
2. Driver = makes two protocols (process ↔ prompt) mutually visible
3. Hook insight: agent 不需要感知 pawl，其 substrate（Stop hook）代理上行路径
4. Foreman-worker 分离：worker can signal, cannot decide
5. Essence 压缩：driver = start only, 其余是 substrate

**-p vs TUI 两种交互模型**

```
-p:  数据通道 = stdin (进程 IO, 一次性)
     交互 = turn-based (stop → reset --step → start, 通过 session resume 继续对话)
     foreman = 被动 (wait → check → done/retry)

TUI: 数据通道 = terminal (send-keys, 持续)
     交互 = real-time (send-keys 实时发指令)
     foreman = 主动 (send → capture → judge → done)
```

统一 driver 的 mode discriminator：`[ -t 0 ]`（stdin 是 tty → TUI，是 pipe → -p）

```jsonc
// -p mode: pipe 存在
"run": "cat $PAWL_TASK_FILE | .pawl/drivers/claude.sh"

// TUI mode: 无 pipe
"run": ".pawl/drivers/claude.sh"
```

### 未解决的矛盾与设计缺口

**1. Viewport trait 的 CRUD 不对称**

viewport trait 有 read（`pawl capture`），没有 write（send）：

```
C: open/execute    ✓
R: read (capture)  ✓
U: send/write      ✗ ← 缺口
D: close           ✓
```

pawl 选择了抽象 read（`capture`），却没抽象 write。但两者本质对称——都是 viewport 操作，都需要从 tmux substrate 抽象。TUI 模式下 foreman 必须穿透到 `tmux send-keys`，这不一致。

**2. `pawl stop` 是否关 viewport/杀进程**

`pawl stop` 改状态（Stopped），但未确认是否关 viewport。如果不关：
- worker 进程还在跑（状态 Stopped 但进程活着）
- foreman 需额外 `tmux kill-pane` 才能真正停止
- 这不像是有意设计

需要检查 `control.rs` 确认行为，可能需要在 stop 时调用 `viewport.close()`。

**3. Agent driver 当前版本 vs 最终版本**

当前已提交的 `claude-driver.sh` 仍是 4 操作 case 语句版本（start/send/stop/read）。S44 讨论得出的 essence 压缩认为 driver = start only，send/stop/read 是 substrate。但这个结论依赖 viewport trait 补全 write（缺口 #1）。

**决策链**：
- 如果补 viewport write → driver 回归 start only（essence）
- 如果不补 → driver 需要保留 send/stop 作为 agent-specific 包装（但有 env var 错位问题）

两条路径互斥。下个 session 需要先决定方向。

**4. Settings.json Stop hook 格式与集成**

已验证 Claude Code `--settings` flag 可用（S44 测试通过）。Hook 格式：
```json
{ "hooks": { "Stop": [{ "hooks": [
  { "type": "command", "command": "pawl done $PAWL_TASK" }
]}]}}
```

当前 claude-driver.sh 未集成 settings.json（只有 -p 模式 flags）。完整版需要：
- co-located `settings.json`（Stop hook → `pawl done`）
- driver 引用 `--settings "$DRIVER_DIR/settings.json"`
- init.rs 同时生成 driver + settings

**5. TUI mode 的 prompt injection 时序**

TUI mode 下 driver 需要在后台注入 prompt（等 TUI 加载后 send-keys）。`sleep 2` 是 magic number：
```bash
{ sleep 2; tmux send-keys -t "$TARGET" -l "$PROMPT"; tmux send-keys -t "$TARGET" C-Enter; } &
claude --session-id "$PAWL_RUN_ID" --settings "$SETTINGS"
```
需要更健壮的等待机制（检测 TUI ready 状态），但这又是 agent-specific。

---

## Previous Sessions (compressed)

### S43: Agent 本质讨论 + 验证
- 从 foreman-worker 模型推导 driver 概念
- Hook insight：agent substrate 代理上行路径
- 首次 driver 实测验证（haiku agent + 简化 echo 命令）
- 确认 S42 viewport fix（32/32 E2E 通过）
- 确认 Issues #1（retry 耗尽审计事件）和 #2（events 全量输出）为 by-design

### S42: Skill doc stress test + viewport close bug fix
- `done.rs` viewport close 时序 fix
- `supervise.md` monitoring 重排
- 5 agent 并行压测 skill 文档

### S41 and earlier
- S41: 9 agent E2E tests
- S39: 32 viewport E2E + 72 sync E2E
- S38: Decouple git, add config.vars
- S37: Skill self-containment
- S36: Less-Is-More Audit
- S33-S35: Agent-First Interface
- S32: Role-based skill architecture

---

## Pending Work

### Driver 最终形态（依赖 viewport write 决策）

1. **决定 viewport write 方向**：补 `pawl send` 原语 or driver 保留 send/stop
2. **确认 `pawl stop` 行为**：检查 control.rs，是否需要加 viewport.close()
3. **集成 settings.json**：claude-driver.sh 引用 co-located settings.json（Stop hook）
4. **TUI mode 实现**：`[ -t 0 ]` 模式检测 + 后台 prompt injection
5. **更新 orchestrate.md**：根据最终 driver 设计更新文档

### 其他

- orchestrate.md 的 Agent Driver 章节目前是 4 操作版本，可能需要根据最终决策简化

## Known Issues

None (kernel level).

## Key File Index

| Area | File |
|------|------|
| CLI definition (14 commands) | `src/cli.rs` |
| PawlError enum (6 variants, exit codes 2-7) | `src/error.rs` |
| Project context, context_for, output_task_state | `src/cmd/common.rs` |
| Status + derive_routing (suggest/prompt) | `src/cmd/status.rs` |
| Execution engine, settle_step, decide() | `src/cmd/start.rs` |
| in_viewport parent process (`pawl _run`) | `src/cmd/run.rs` |
| Done/approve handler | `src/cmd/done.rs` |
| Stop/Reset handler | `src/cmd/control.rs` |
| Wait (poll with Timeout) | `src/cmd/wait.rs` |
| Entry point, PawlError → text stderr | `src/main.rs` |
| Project root discovery, task name validation | `src/util/project.rs` |
| Context builder (expand/to_env_vars/var_owned) | `src/util/variable.rs` |
| Shell command execution | `src/util/shell.rs` |
| Config model + Step + vars (IndexMap) | `src/model/config.rs` |
| Event model + replay + count_auto_retries | `src/model/event.rs` |
| TaskState, TaskStatus, StepStatus | `src/model/state.rs` |
| Templates (config + skill + references + driver) | `src/cmd/templates/` |
| Viewport trait + TmuxViewport | `src/viewport/` |
| E2E tests (sync paths, 72 tests) | `tests/e2e.sh` |
| E2E tests (viewport paths, 32 tests) | `tests/e2e-viewport.sh` |
| E2E tests (agent paths, 9 tests) | `tests/e2e-agent.sh` |
