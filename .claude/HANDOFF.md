# Session Handoff

## Current Session (S33): Agent-First Output Restructuring

### What changed

将单一不变量 `state = replay(log)` 延伸到 CLI 边界：`output = serialize(replay(log))`。

**stdout = JSON/JSONL 数据，stderr = 进度/错误。** 所有 `--json`/`--jsonl` flags 删除（JSON 是默认，不是选项）。

**写命令 JSON 输出**（6 个命令）

| 命令 | stdout |
|------|--------|
| `start/done/stop/reset` | `output_task_state()` — task/status/run_id/current_step/step_name/total_steps/message/retry_count/last_feedback |
| `create` | task/task_file/depends |
| `init` | pawl_dir/config |

所有进度消息 (`println!` → `eprintln!`) 移到 stderr。

**读命令删除人类层**（~395 行删除）

- `status.rs`: 删除 `show_all_tasks`/`show_task_detail`/`format_waiting_reason`/`format_duration`/`truncate` (5 个函数)
- `log.rs`: 删除 `print_event` (108 行) + `run_jsonl` + `current_run_events`
- `capture.rs`: 删除人类分支，修复 1-based → 0-based bug
- `model/mod.rs`: 删除未使用的 `StepStatus` re-export

**变量增丰**（10 → 12 个）

| 新变量 | 用途 |
|--------|------|
| `${retry_count}` / `$PAWL_RETRY_COUNT` | 当前步骤的自动重试次数 |
| `${last_verify_output}` / `$PAWL_LAST_VERIFY_OUTPUT` | 上次失败输出 |

4 个注入点: execute loop, run_verify, run_in_viewport, spawn_event_hook。

`extract_step_context()` 从 `status.rs` 移到 `common.rs`（共享于 status + output_task_state）。

orchestrate.md: grep-based retry feedback 样板 → `$PAWL_LAST_VERIFY_OUTPUT` 直接使用。

### Net result

- 37/37 测试通过，零 warnings
- 所有 `println!` 审计通过：仅用于 JSON/JSONL 数据输出
- 所有 1-based 索引仅出现在 `eprintln!`（stderr 进度）

---

## Previous Sessions (compressed)

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
| Project context, extract_step_context, output_task_state | `src/cmd/common.rs` |
| Execution engine, settle_step pipeline, decide() | `src/cmd/start.rs` |
| in_viewport parent process (`pawl _run`) | `src/cmd/run.rs` |
| Done/approve handler | `src/cmd/done.rs` |
| Status (JSON output, no human layer) | `src/cmd/status.rs` |
| Context builder (build/var/get/expand/to_env_vars/extend) | `src/util/variable.rs` |
| Event model + replay + count_auto_retries | `src/model/event.rs` |
| TaskState, TaskStatus (Display+FromStr), StepStatus | `src/model/state.rs` |
| Viewport trait + factory | `src/viewport/mod.rs` |
| TmuxViewport implementation | `src/viewport/tmux.rs` |
| Config model + Step | `src/model/config.rs` |
| Init (scaffold + templates) | `src/cmd/init.rs` |
| Templates (config + skill + references) | `src/cmd/templates/` |
