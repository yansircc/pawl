# Session Handoff

## Current Session (S37): Skill Self-Containment + human→manual

### Key Insight

Skill 文档应该是每个角色的唯一信息源，zero jumps。`-h` 和 config.jsonc 注释不应该承担文档职责 — 它们只是使用时的辅助，不是学习入口。

`"human"` 一词暗示只有人类能做 verify/on_fail 决策，但实际上 supervisor agent 同样可以。替换为 `"manual"` — 对立面是 automated（retry / shell command），不隐含操作者身份。

### What changed (S37)

**Skill 文档自完备（zero jumps from SKILL.md）**
- SKILL.md: 删 `pawl --help` 指针，删 config.jsonc 指针，内联 states + indexing
- author.md: 加 `pawl create` 入口命令
- supervise.md: 加 States 段 + Status Fields 段（完整字段列表）
- orchestrate.md: 加 Top-Level Options + Step Properties 表 + Variables 列表 + Design Rules + Event hook event types

**config.jsonc 空画布**
- 46 行 → 4 行。删预置 git worktree 骨架 + 35 行文档注释
- 强制 agent 读 orchestrate.md 设计自己的 workflow，不再开箱即用

**CLI -h 全面精简**
- 删除所有 8 个 `after_help` 块。cli.rs: 157 → 134 行
- `-h` 只保留 clap 自动生成的 usage + arg descriptions

**`"human"` → `"manual"` 全局替换**
- 配置值: `verify: "manual"`, `on_fail: "manual"`
- 事件 reason: `verify_manual`, `on_fail_manual`
- 枚举: `ManualNeeded`, `FailPolicy::Manual`
- 影响 13 个文件（src + templates + README）

---

## Previous Sessions (compressed)

### S36: Less-Is-More Audit
- 三个生成元缺刹车 → less-is-more 作为停机条件
- PawlError 精简（删 Serialize/suggest）。error.rs: 77→43 行
- derive_routing() 移到 status.rs（唯一消费者）
- Skill 文档开始内联（author.md frontmatter, supervise.md suggest/prompt）

### S33-S35: Agent-First Interface
- S33: stdout=JSON/JSONL, stderr=progress。~395 行人类层删除。output_task_state() 统一输出。
- S34: PawlError enum → exit codes 2-7。Viewport trait 精简。
- S35: derive_routing() 自路由协议。

### S32: Role-Based Skill Architecture
- SKILL.md: 249 → 29 行。3 role references。config.jsonc 自文档化。

### S31: Trust the Substrate
- SKILL.md: 388 → 248 行。CLI `after_help` 自文档化。

### S30 and earlier
- S30: 解耦 Claude Code（删 claude_command + ai-helpers.sh + plan-worker.mjs）
- S28: 结构压缩（settle_step pipeline, Display+FromStr, context_for/step_name）
- S27: Viewport trait + TmuxViewport
- S25-26: Rename wf → pawl, crates.io publish
- S1-24: Architecture evolution, Foreman mode, first principles

---

## Pending Work

- **`${session_id}` variable**: pawl 生成稳定 UUID 暴露为变量，消除 worker session 管理样板

## Known Issues

- **retry exhaustion has no audit event**: no event when transitioning from retry to terminal state
- `pawl events` outputs full history (not filtered by current run), inconsistent with `pawl log --all`
- **clap 4.5 long_about broken**: doc comments don't set `long_about`; use `after_help` attribute (now moot — all after_help deleted in S37)

## Key File Index

| Area | File |
|------|------|
| CLI definition (14 commands), usage only | `src/cli.rs` |
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
