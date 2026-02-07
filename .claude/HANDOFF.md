# Session Handoff

## 本次 Session 完成的工作

### Session 15: Foreman 文档全面完善

**新增 3 份文档**（嵌入 init.rs，`wf init` 自动生成到 `.wf/lib/`）：

| 产物 | 位置 | 行数 | 覆盖内容 |
|------|------|------|---------|
| Task.md 编写指南 | `.wf/lib/task-authoring-guide.md` | 314 | 双重身份（人类文档+AI prompt）、frontmatter 完整字段、五要素写作法、迭代反馈模式、skip 用法、3 个完整示例 |
| AI Worker 集成指南 | `.wf/lib/ai-worker-guide.md` | 275 | ai-helpers.sh 函数参考、wrapper.sh 模式、Claude 非交互模式、会话续接流程、Event Hook 通知闭环、环境变量表、实战配置示例 |
| Foreman Guide 增强版 | `.wf/lib/foreman-guide.md` | 468 | 原有内容全保留 + 创建任务章节 + JSON 输出格式完整 schema + 状态决策速查表 + 故障排查 + 相关文档索引 |

**改进 `wf create` 模板** (create.rs)：
- 新增 `skip` 字段注释示例（frontmatter）
- 新增 AI Worker prompt 使用提示
- 默认模板结构改为：目标 / 约束 / 验收标准

**init.rs 重构** — `include_str!` 拆分：
- 模板文件独立到 `src/cmd/templates/`（config.jsonc, ai-helpers.sh, 3 份 guide）
- init.rs 从 ~640 行降至 205 行
- Hook 模板（verify-stop.sh, settings.json, gitignore）保持内联，下轮处理

**技术指标**: 36 tests, zero warnings, `cargo install --path .` 完成

---

## 历史 Session

### Session 14: E2E 包工头测试 + 痛点修复 + Foreman Guide
- 8步 × 3task × 16场景 E2E、6个UX修复、初版 Foreman Guide + Config 模板

### Session 13: P0/P1/P2 重构 + Greenfield
- resolve/dispatch 分离（7 单元测试）、WindowLost 统一、wait.rs 走 Project API

### Session 12: 第一性原理审视 + VerifyFailed 消除
- 事件 11→10，StepCompleted 统一发射，三 agent 审计

### Session 9-11: 辩论驱动改进 + E2E
- Step 0-based 统一、start --reset、events --follow、log 当前轮

### Session 5-8: Foreman 模式 + P1-P5
- 非交互 Claude、wrapper.sh、事件 hook、并发 task

### Session 1-4: 架构演进
- TUI 删除 → Event Sourcing → Step 模型 → Unified Pipeline → E2E 测试

---

## 已知监控项

- **on_exit + wf done 双权威竞态**: in_window 步骤两个裁决者可同时触发 (V7 缓解但未完全消除)
- **on_exit 丢失 RunOutput**: in_window 进程退出无 stdout/stderr/duration
- **retry 耗尽无审计事件**: 从 retry 转终态时无事件记录 (V10)
- **verify:human 崩溃瞬态**: 两个 append 间崩溃窗口极小 (V5)
- `wf events` 输出全部历史（不按当前轮过滤），与 `wf log --all` 不一致

## 关键文件索引

| 功能 | 文件 |
|------|------|
| CLI 定义（14 命令） | `src/cli.rs` |
| 配置模型（Step 4 属性） | `src/model/config.rs` |
| 事件模型 + replay + count_auto_retries（10 种） | `src/model/event.rs` |
| 状态投影类型 | `src/model/state.rs` |
| 任务定义（含 skip） | `src/model/task.rs` |
| 执行引擎 + resolve/dispatch 管线 | `src/cmd/start.rs` |
| 审批命令（done） | `src/cmd/approve.rs` |
| 控制命令（stop/reset/on_exit） | `src/cmd/control.rs` |
| 状态输出（retry_count/last_feedback/waiting reason） | `src/cmd/status.rs` |
| 日志输出（当前轮/全历史/--jsonl） | `src/cmd/log.rs` |
| 统一事件流（--follow） | `src/cmd/events.rs` |
| 等待命令（Project API + check_window_health） | `src/cmd/wait.rs` |
| 初始化（include_str! 加载模板） | `src/cmd/init.rs` |
| 模板文件（config/guides/ai-helpers） | `src/cmd/templates/` |
| 任务创建（含改进模板） | `src/cmd/create.rs` |
| 公共工具（事件读写、钩子、check_window_health） | `src/cmd/common.rs` |
| tmux 工具（窗口后台创建） | `src/util/tmux.rs` |
| git 工具（branch_exists） | `src/util/git.rs` |
| 变量上下文 | `src/util/variable.rs` |
| 包工头操作手册 (增强版) | `.wf/lib/foreman-guide.md` |
| Task.md 编写指南 | `.wf/lib/task-authoring-guide.md` |
| AI Worker 集成指南 | `.wf/lib/ai-worker-guide.md` |
| 项目概述 | `.claude/CLAUDE.md` |
