# Session Handoff

## Current Session (S36): Less-Is-More Audit

### Key Insight

三个生成元（separate/derive/trust）是发动机，但没有刹车。S33-S35 每轮局部优雅、全局膨胀。S36 引入 less-is-more 作为停机条件：每个机制必须由当前使用证明其存在，不是由设计美学。

两条派生规则：
1. **去重收益 ∝ 变更频率** — 稳定事实内联，易变细节才用指针
2. **每个机制只在其职责范围内** — 错误报告，状态路由，写命令确认

Agent UX 发现：progressive disclosure 是人类模式。Agent 要 flat, complete, searchable。

### What changed (S36)

**代码精简（retroactive S33-S35 cleanup）**
- `PawlError`: 删 `Serialize`, `#[serde]`, `suggest()`, `Timeout.task`。error.rs: 77 → 43 行
- `main.rs`: JSON error → `eprintln!("{pe}")` + exit code。8 → 3 行 error handling
- `output_task_state()`: 删 suggest/prompt。写命令只报告状态，不路由
- `derive_routing()`: 从 `Project` (common.rs) 移到 `status.rs` — 唯一消费者
- suggest/prompt 单一发射点：`pawl status` 输出

**Skill 文档自包含**
- SKILL.md: "Errors are structured" → "stderr = plain text"
- author.md: 内联 frontmatter 字段（was: "run `pawl create --help`"）
- supervise.md: 内联 suggest/prompt 语义，补充 events --follow / wait / capture
- orchestrate.md: 不动（已自包含）

**CLI -h 精简**
- 顶层: 删 STEP PROPERTIES + VARIABLES（在 config.jsonc 注释里）
- `start -h`: 删内部管线描述（settle_step pipeline）
- `status -h`: 加 suggest/prompt 字段说明

**设计哲学更新**
- CLAUDE.md: 新增 "Stop condition: less-is-more" + "Agent UX ≠ Human UX"

---

## Previous Sessions (compressed)

### S33-S35: Agent-First Interface
- S33: stdout=JSON/JSONL, stderr=progress。~395 行人类层删除。output_task_state() 统一输出。
- S34: PawlError enum → exit codes 2-7。~35 bail! 转换。Viewport trait 精简。
- S35: derive_routing() 自路由协议。~~JSON errors, suggest in errors~~ (reverted S36)

### S32: Role-Based Skill Architecture
- SKILL.md: 249 → 29 行。3 role references。config.jsonc 自文档化。

### S31: Trust the Substrate
- SKILL.md: 388 → 248 行。CLI `after_help` 自文档化。

### S30: Decouple Claude Code
- 删除 `claude_command` + `ai-helpers.sh` + `plan-worker.mjs`

### S28: Structure Compression
- `settle_step()` pipeline, `Display`+`FromStr`, `context_for/step_name/worktree_path`

### S27: Viewport Trait
- `Viewport` trait + `TmuxViewport` impl

### S25-26: Rename wf → pawl, crates.io publish

### S1-24: Architecture evolution, Foreman mode, first principles

---

## Pending Work

- **`${session_id}` variable**: pawl 生成稳定 UUID 暴露为变量，消除 worker session 管理样板

## Known Issues

- **retry exhaustion has no audit event**: no event when transitioning from retry to terminal state
- `pawl events` outputs full history (not filtered by current run), inconsistent with `pawl log --all`
- **clap 4.5 long_about broken**: doc comments don't set `long_about`; use `after_help` attribute

## Key File Index

| Area | File |
|------|------|
| CLI definition (14 commands) + help text | `src/cli.rs` |
| PawlError enum (6 variants, exit codes 2-7) | `src/error.rs` |
| Project context, output_task_state | `src/cmd/common.rs` |
| Status + derive_routing (suggest/prompt) | `src/cmd/status.rs` |
| Execution engine, settle_step, decide() | `src/cmd/start.rs` |
| in_viewport parent process (`pawl _run`) | `src/cmd/run.rs` |
| Done/approve handler | `src/cmd/done.rs` |
| Wait (poll with Timeout) | `src/cmd/wait.rs` |
| Entry point, PawlError → text stderr | `src/main.rs` |
| Context builder (expand/to_env_vars/extend) | `src/util/variable.rs` |
| Event model + replay + count_auto_retries | `src/model/event.rs` |
| TaskState, TaskStatus, StepStatus | `src/model/state.rs` |
| Config model + Step | `src/model/config.rs` |
| Templates (config + skill + references) | `src/cmd/templates/` |
| Viewport trait + TmuxViewport | `src/viewport/` |
