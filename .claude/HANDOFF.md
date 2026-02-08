# Session Handoff

## Current Session (S35): Self-Routing Protocol + Agent-First Identity

### Key Insight

pawl 的消费者是 agent，不是 human。这条认知串起了 S33-S35 的全部重构：

- S33: stdout=JSON, stderr=progress（机器可读输出）
- S34: PawlError → JSON stderr + exit codes（结构化错误）
- S35: suggest/prompt（自路由协议，消除 agent 猜测）

pawl 是协程，不是 daemon——它 yield 出路由提示，由外部 agent 决定是否 resume。真正机械的部分（retry）已在内部自动执行；到达 suggest 的都是内部自动化已穷尽的情况。

### What changed (S34 + S35)

**S34: Structured Errors**
- `PawlError` enum (6 variants) → JSON stderr + exit codes 2-7
- ~35 `bail!` → `PawlError` conversions across 12 files
- Viewport trait: `send` → `execute`, `is_active` promoted to trait
- `"task"` → `"name"` field unification; `events --type` filter

**S35: Self-Routing Protocol**
- `PawlError::suggest()` — 从错误变体数据派生恢复命令
- `Project::derive_routing()` — (status, message, task) → (suggest, prompt)
- `suggest` = 机械命令（agent 直接执行），`prompt` = 需判断力（agent 评估后决定）
- `pawl done` 永不出现在 suggest（需要判断，不是路由）
- `Timeout` 变体增加 `task` 字段用于 suggest 派生
- `output_task_state()` + `TaskSummary`/`TaskDetail` 包含 suggest/prompt
- supervise.md: 51 → 33 行（Status Decision Table 删除，已编码在 derive_routing）
- 文档全面更新：CLAUDE.md 新增 Agent-First Interface 章节，README/SKILL.md 反映 agent-first 定位

---

## Previous Sessions (compressed)

### S33: Agent-First Output Restructuring
- `output = serialize(replay(log))`. stdout=JSON/JSONL, stderr=progress. ~395 行人类层删除。
- `output_task_state()` 统一 6 个写命令输出。新变量 `${retry_count}`, `${last_verify_output}`。

### S32: Role-Based Skill Architecture
- SKILL.md: 249 → 29 行 (routing only)。3 role references 创建。config.jsonc 自文档化。

### S31: Trust the Substrate
- SKILL.md: 388 → 248 行。CLI 通过 `after_help` 自文档化。

### S30: Decouple Claude Code
- 删除 `claude_command` + `ai-helpers.sh` + `plan-worker.mjs`

### S28: Structure Compression
- `settle_step()` pipeline, `Display`+`FromStr`, `context_for/step_name/worktree_path`

### S27: Viewport Trait
- `Viewport` trait + `TmuxViewport` impl, `multiplexer` → `viewport`

### S25-26: Rename wf → pawl, crates.io publish

### S1-24: Architecture evolution, Foreman mode, first principles, resolve/dispatch refactor

---

## Pending Work

- **`${session_id}` variable**: pawl 生成稳定 UUID 暴露为变量，消除 worker 自行管理 session-id 的样板代码。

## Known Issues

- **retry exhaustion has no audit event**: no event emitted when transitioning from retry to terminal state
- `pawl events` outputs full history (not filtered by current run), inconsistent with `pawl log --all`
- **clap 4.5 long_about broken**: doc comments don't set `long_about` correctly in clap 4.5.57; `after_help` attribute is the workaround

## Key File Index

| Area | File |
|------|------|
| CLI definition (14 commands) + help text | `src/cli.rs` |
| PawlError enum, exit_code(), suggest() | `src/error.rs` |
| Project context, output_task_state, derive_routing | `src/cmd/common.rs` |
| Execution engine, settle_step pipeline, decide() | `src/cmd/start.rs` |
| Status (JSON output with suggest/prompt) | `src/cmd/status.rs` |
| in_viewport parent process (`pawl _run`) | `src/cmd/run.rs` |
| Done/approve handler | `src/cmd/done.rs` |
| Wait (poll with Timeout suggest) | `src/cmd/wait.rs` |
| Entry point, PawlError → JSON stderr + suggest | `src/main.rs` |
| Context builder (build/var/get/expand/to_env_vars/extend) | `src/util/variable.rs` |
| Event model + replay + count_auto_retries | `src/model/event.rs` |
| TaskState, TaskStatus (Display+FromStr), StepStatus | `src/model/state.rs` |
| Viewport trait + factory | `src/viewport/mod.rs` |
| TmuxViewport implementation | `src/viewport/tmux.rs` |
| Config model + Step | `src/model/config.rs` |
| Init (scaffold + templates) | `src/cmd/init.rs` |
| Templates (config + skill + references) | `src/cmd/templates/` |
