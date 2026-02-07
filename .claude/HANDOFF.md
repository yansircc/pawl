# Session Handoff

## 本次 Session 完成的工作

### 辩论驱动改进：5 项实施

通过 3-agent 对抗辩论（Event Stream First vs API Contract First vs 人格拆分批评），发现"哪个是 P0"问题框架有误——三个改进不在同一量级，应按成本分层直接执行。随后全部实施。

**改动一览**：

| # | 改动 | 文件 | 行数 |
|---|------|------|------|
| 1 | Step 编号统一：`--json` 和 `--step` 改为 0-based | `status.rs`, `log.rs`, `cli.rs` | ~10 |
| 2 | 修复 `start.rs:22` 错误消息输出 0-based bug | `start.rs` | 1 |
| 3 | `wf start --reset` 一步完成 reset+start | `cli.rs`, `mod.rs`, `start.rs` | ~15 |
| 4 | `wf events [task] [--follow]` 统一事件流 | 新 `events.rs` + `cli.rs`, `mod.rs`, `Cargo.toml` | ~150 |
| 5 | `wf log --all` 默认当前轮，`--all-runs` 显全历史 | `log.rs`, `cli.rs`, `mod.rs` | ~30 |

**辩论核心发现**：
- Step 编号不一致是系统性 bug（25+ 处 `+1` 转换漏了 1 处 = 实证）
- Event stream 是多任务 reactive 编排的不可替代原语
- `wf start --reset` 是最高 ROI 的便利改进
- 三者互不阻塞，不应排序竞争，应按成本分层直接执行

零 warning，29/29 测试通过，E2E 全部验证。新依赖：`notify = "7"`。

---

## 历史 Session

### Session 8: P1-P5 改进路线图 + Greenfield 清理
- `wf log --jsonl`、`wf wait --until` 多状态、`wf status --json` 增强
- `done()` 统一管线、`wf init` 生成 ai-helpers.sh
- Greenfield 清理 4 项

### Session 7: 零基审查 + 代码质量修复
- 零基审查 11 项裁定（5 同意/6 驳回），净删 ~18 行

### Session 5-6: Foreman 模式 + 非交互 Claude 闭环
- worker.sh / wrapper.sh / verify-ai.sh 模式验证
- 事件 hook 通知闭环、并发互斥

### Session 1-4: 架构演进
- TUI 删除 → Event Sourcing → Step 模型 → Unified Pipeline → E2E 测试

---

## 已知监控项

- `extract_step_context()` (status.rs) 与 `count_auto_retries()` (start.rs) 逻辑相似，两处重复未达 greenfield 3 次阈值，暂保留
- `wf events` 输出全部历史事件（不按当前轮过滤），与 `wf log --all` 行为不一致。如需统一可后续加 `--current-run` 选项

## 关键文件索引

| 功能 | 文件 |
|------|------|
| CLI 定义（14 命令） | `src/cli.rs` |
| 配置模型（Step 4 属性） | `src/model/config.rs` |
| 事件模型 + replay（11 种） | `src/model/event.rs` |
| 状态投影类型 | `src/model/state.rs` |
| 任务定义（含 skip） | `src/model/task.rs` |
| 执行引擎 + 统一管线 | `src/cmd/start.rs` |
| 审批命令（done，统一管线） | `src/cmd/approve.rs` |
| 控制命令（stop/reset/on_exit） | `src/cmd/control.rs` |
| 状态输出（含 retry_count/last_feedback） | `src/cmd/status.rs` |
| 日志输出（当前轮/全历史/--jsonl） | `src/cmd/log.rs` |
| 统一事件流（--follow 实时监听） | `src/cmd/events.rs` |
| 等待命令（多状态支持） | `src/cmd/wait.rs` |
| 初始化（含 lib 模板生成） | `src/cmd/init.rs` |
| 公共工具（事件读写、钩子） | `src/cmd/common.rs` |
| 变量上下文 | `src/util/variable.rs` |
| 项目概述 | `.claude/CLAUDE.md` |
