# Session Handoff

## 本次 Session 完成的工作

### Session 16: 文档重构为 Claude Code Skill

将 wf 操作文档从 `.wf/lib/` 重构为 `.claude/skills/wf/` Claude Code skill 体系。

**新增**:
- `src/cmd/templates/wf-skill.md` — SKILL.md 参考卡 (~100 行)，包含 CLI 命令表、Step 类型速查、状态机、Config 格式、变量表、Event Hooks

**精简**:
- `config.jsonc` 模板: 144→15 行 (删除全部教程注释，纯配置)
- `foreman-guide.md`: 369→291 行 (删除 SKILL.md 已覆盖的速查表、消除支撑文件间互引)
- `ai-worker-guide.md`: 274→246 行 (Claude 非交互模式精简、环境变量表指向 SKILL.md)

**修改**:
- `init.rs`: `create_lib_files()` 只保留 ai-helpers.sh，新增 `create_skill_files()` 写 4 文件到 `.claude/skills/wf/`
- `create.rs`: 路径引用 `.wf/lib/` → `.claude/skills/wf/`
- CLAUDE.md / HANDOFF.md / README.md: 目录结构和路径索引更新

**信息架构**: SKILL.md → 3 个独立叶子 guide（无互引），符合三条规则（只放关键参考、最多 2 层、支撑文件不互引）

**技术指标**: 36 tests, zero warnings, 净减 330 行

---

## 历史 Session

### Session 15: Foreman 文档全面完善
- 新增 3 份指南文档 (task-authoring/ai-worker/foreman-guide)，改进 create 模板，init.rs include_str! 拆分

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
| 初始化（include_str! 加载模板 + skill 生成） | `src/cmd/init.rs` |
| 模板文件（config/skill/guides/ai-helpers） | `src/cmd/templates/` |
| 任务创建（含改进模板） | `src/cmd/create.rs` |
| 公共工具（事件读写、钩子、check_window_health） | `src/cmd/common.rs` |
| tmux 工具（窗口后台创建） | `src/util/tmux.rs` |
| git 工具（branch_exists） | `src/util/git.rs` |
| 变量上下文 | `src/util/variable.rs` |
| wf Skill 参考卡 | `.claude/skills/wf/SKILL.md` |
| 包工头操作手册 | `.claude/skills/wf/foreman-guide.md` |
| Task.md 编写指南 | `.claude/skills/wf/task-authoring-guide.md` |
| AI Worker 集成指南 | `.claude/skills/wf/ai-worker-guide.md` |
| 项目概述 | `.claude/CLAUDE.md` |
