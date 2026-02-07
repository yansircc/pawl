use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::util::git::get_repo_root;

const WF_DIR: &str = ".wf";
const CONFIG_FILE: &str = "config.jsonc";
const TASKS_DIR: &str = "tasks";
const HOOKS_DIR: &str = "hooks";
const LIB_DIR: &str = "lib";

const DEFAULT_CONFIG: &str = r#"{
  // ============================================
  // wf config — Workflow Configuration
  // ============================================
  //
  // 阅读 .wf/lib/foreman-guide.md 了解完整的包工头操作指南。
  //
  // ============================================
  // 基础配置
  // ============================================

  // tmux session 名称（默认: 项目目录名）
  // "session": "my-project",

  // Worktree 存放目录（相对于 repo root，默认: ".wf/worktrees"）
  // "worktree_dir": ".wf/worktrees",

  // 基础分支（用于创建任务分支的起点，默认: "main"）
  // "base_branch": "main",

  // ============================================
  // Workflow — 步骤序列
  // ============================================
  //
  // 所有 task 共享同一个 workflow。每个 step 有 4 个正交属性:
  //
  //   name     (必须)  步骤名称，用于显示和 task 级别 skip
  //   run      (可选)  shell 命令。省略 = gate 步骤（暂停等 wf done）
  //   verify   (可选)  "human" = 人工审核; 或 shell 命令（exit 0 = 通过）
  //   on_fail  (可选)  "retry" = 自动重试; "human" = 暂停等人工决策
  //   in_window(可选)  true = 在 tmux 窗口中执行，等 wf done 标记完成
  //   max_retries(可选) on_fail="retry" 时的最大重试次数（默认: 3）
  //
  // ---- Step 类型速查 ----
  //
  //  普通步骤     { "name": "x", "run": "cmd" }
  //    → 同步执行，exit 0 自动前进，exit != 0 则 Failed
  //
  //  Gate 步骤    { "name": "x" }
  //    → 无 run，立即暂停，等 `wf done <task>` 放行
  //    → 用途: 人工检查点、外部依赖确认
  //
  //  自动验证     { "name": "x", "run": "cmd", "verify": "test.sh" }
  //    → run 成功后执行 verify 脚本，exit 0 = 通过
  //    → 搭配 on_fail 使用: 验证失败时自动重试或等人工
  //
  //  人工审核     { "name": "x", "run": "cmd", "verify": "human" }
  //    → run 成功后暂停，等人工审查产物后 `wf done`
  //
  //  自动重试     { ..., "on_fail": "retry", "max_retries": 3 }
  //    → 失败后自动重试最多 N 次，耗尽则 Failed
  //
  //  人工介入     { ..., "on_fail": "human" }
  //    → 失败后暂停，等人工分析后 `wf done`(放行) 或 `wf reset --step`(重试)
  //
  //  窗口任务     { "name": "x", "run": "cmd", "in_window": true }
  //    → 在 tmux 窗口中后台执行（不抢焦点）
  //    → 用 `wf capture` 查看进展，`wf done` 标记完成
  //    → 典型用途: AI agent 长时间开发
  //
  // ---- 变量 ----
  //
  // 所有 run/verify 命令中可用 ${var}，子进程中为 WF_VAR 环境变量:
  //   ${task}        任务名
  //   ${branch}      wf/{task}
  //   ${worktree}    {repo_root}/{worktree_dir}/{task}
  //   ${window}      同 task 名
  //   ${session}     tmux session 名
  //   ${repo_root}   仓库根目录
  //   ${step}        当前步骤名
  //   ${step_index}  当前步骤索引 (0-based)
  //   ${base_branch} 基础分支名
  //   ${log_file}    .wf/logs/{task}.jsonl
  //   ${task_file}   .wf/tasks/{task}.md
  //
  // ---- 设计建议 ----
  //
  // 1. cleanup 步骤的命令末尾加 `; true`，确保清理失败不阻塞
  // 2. in_window 步骤不要和 verify 组合 — verify 在 wf done 时运行，
  //    但 in_window 的 stdout/stderr 不会被捕获，verify 拿不到产物
  // 3. 用 task 的 skip 字段跳过不适用的步骤，而不是改 workflow
  // 4. verify 脚本应该幂等 — 可能被重试多次执行

  "workflow": [
    // 1. 资源准备 — 创建隔离的工作环境
    { "name": "Create branch", "run": "git branch ${branch} ${base_branch}" },
    { "name": "Create worktree", "run": "git worktree add ${worktree} ${branch}" },

    // 2. 开发 — AI agent 在 tmux 窗口中执行
    {
      "name": "Develop",
      "run": "source ${repo_root}/.wf/lib/ai-helpers.sh && run_ai_worker",
      "in_window": true
    },

    // 3. 审核 — 人工 review 开发产物
    { "name": "Review", "run": "echo 'Changes in ${worktree}:'  && cd ${worktree} && git diff --stat HEAD~1", "verify": "human" },

    // 4. 测试 — 自动验证 + 失败重试
    {
      "name": "Test",
      "run": "cd ${worktree} && make test",
      "on_fail": "retry",
      "max_retries": 2
    },

    // 5. 合并
    {
      "name": "Merge",
      "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge from wf'"
    },

    // 6. 清理 — 命令末尾 `; true` 确保即使清理失败也不阻塞
    {
      "name": "Cleanup",
      "run": "git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true"
    }
  ]

  // ============================================
  // Event Hooks（可选）
  // ============================================
  //
  // 事件触发的 shell 命令，fire-and-forget（异步执行，不阻塞主流程）。
  // key = 事件类型名 (snake_case)，在 append_event() 时自动触发。
  //
  // 可用事件:
  //   task_started    — 任务启动
  //   step_completed  — 步骤完成 (额外变量: ${exit_code}, ${duration})
  //   step_waiting    — 步骤暂停等待 (额外变量: ${reason})
  //   step_approved   — 步骤被批准
  //   step_skipped    — 步骤被跳过
  //   step_reset      — 步骤被重置 (额外变量: ${auto})
  //   window_launched — tmux 窗口已创建
  //   window_lost     — tmux 窗口丢失
  //   task_stopped    — 任务被停止
  //   task_reset      — 任务被完全重置
  //
  // 示例:
  // "on": {
  //   "step_completed": "echo '[${task}] step ${step} exit=${exit_code}' >> ${repo_root}/wf.log",
  //   "step_waiting": "echo '[${task}] waiting at ${step}: ${reason}' >> ${repo_root}/wf.log",
  //   "window_lost": "echo '[${task}] ALERT: window lost at ${step}' >> ${repo_root}/wf.log"
  // }
}
"#;

const VERIFY_STOP_SCRIPT: &str = r#"#!/bin/bash
# wf verify script - auto generated
INPUT=$(cat)
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
TRANSCRIPT=$(echo "$INPUT" | jq -r '.transcript_path // empty')

[ -z "$TRANSCRIPT" ] || [ ! -f "$TRANSCRIPT" ] && exit 0

STATE_FILE="/tmp/wf-verify-${SESSION_ID}.state"
LAST_LINE=0; [ -f "$STATE_FILE" ] && LAST_LINE=$(cat "$STATE_FILE")
CURRENT_LINE=$(wc -l < "$TRANSCRIPT" | tr -d ' ')
[ "$CURRENT_LINE" -le "$LAST_LINE" ] && exit 0

tail -n +$((LAST_LINE + 1)) "$TRANSCRIPT" | grep -q 'wf done' && { rm -f "$STATE_FILE"; exit 0; }

echo "$CURRENT_LINE" > "$STATE_FILE"
TASK="${WF_TASK:-task}"
echo "{\"decision\":\"block\",\"reason\":\"【自检】请确认任务完成后执行 wf done ${TASK}\"}"
exit 0
"#;

const SETTINGS_JSON_TEMPLATE: &str = r#"{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "REPO_ROOT/.wf/hooks/verify-stop.sh"
          }
        ]
      }
    ]
  }
}
"#;

const GITIGNORE_ENTRIES: &str = r#"
# wf - Workflow Task Runner
.wf/*
!.wf/tasks/
!.wf/config.jsonc
!.wf/hooks/
!.wf/lib/
"#;

const AI_HELPERS_TEMPLATE: &str = r#"#!/usr/bin/env bash
# .wf/lib/ai-helpers.sh — AI worker helper functions
# Source this file in your worker/wrapper scripts:
#   source "$(dirname "$0")/../lib/ai-helpers.sh"

set -euo pipefail

# Extract the most recent session_id from a task's JSONL log.
# Usage: extract_session_id <jsonl_file>
# Returns empty string if no session_id found.
extract_session_id() {
    local log_file="${1:?Usage: extract_session_id <jsonl_file>}"
    [ -f "$log_file" ] || { echo ""; return 0; }
    grep -o '"session_id":"[^"]*"' "$log_file" | tail -1 | cut -d'"' -f4
}

# Extract the most recent failure feedback from a task's JSONL log.
# Looks for step_completed events with exit_code != 0 and extracts stderr.
# Usage: extract_feedback <jsonl_file> [step_index]
# Returns empty string if no feedback found.
extract_feedback() {
    local log_file="${1:?Usage: extract_feedback <jsonl_file> [step_index]}"
    local step_idx="${2:-}"
    [ -f "$log_file" ] || { echo ""; return 0; }

    if [ -n "$step_idx" ]; then
        grep '"type":"step_completed"' "$log_file" \
            | grep "\"step\":${step_idx}" \
            | jq -r 'select(.exit_code != 0) | .stderr // empty' 2>/dev/null \
            | tail -1
    else
        grep '"type":"step_completed"' "$log_file" \
            | jq -r 'select(.exit_code != 0) | .stderr // empty' 2>/dev/null \
            | tail -1
    fi
}

# AI worker wrapper: handles fresh start vs resume, injects feedback.
# Usage: run_ai_worker [options]
#   --log-file <path>     JSONL log file (default: $WF_LOG_FILE)
#   --task-file <path>    Task markdown file (default: $WF_TASK_FILE)
#   --tools <tools>       Comma-separated tool list (default: Bash,Read,Write)
#   --claude-cmd <cmd>    Claude command (default: claude)
#   --extra-args <args>   Extra arguments to pass to claude
run_ai_worker() {
    local log_file="${WF_LOG_FILE:-}"
    local task_file="${WF_TASK_FILE:-}"
    local tools="Bash,Read,Write"
    local claude_cmd="claude"
    local extra_args=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --log-file)   log_file="$2"; shift 2 ;;
            --task-file)  task_file="$2"; shift 2 ;;
            --tools)      tools="$2"; shift 2 ;;
            --claude-cmd) claude_cmd="$2"; shift 2 ;;
            --extra-args) extra_args="$2"; shift 2 ;;
            *) echo "Unknown option: $1" >&2; return 1 ;;
        esac
    done

    [ -z "$log_file" ] && { echo "Error: --log-file or WF_LOG_FILE required" >&2; return 1; }
    [ -z "$task_file" ] && { echo "Error: --task-file or WF_TASK_FILE required" >&2; return 1; }

    local session_id
    session_id=$(extract_session_id "$log_file")

    local feedback
    feedback=$(extract_feedback "$log_file")

    if [ -n "$session_id" ]; then
        # Resume existing session with feedback
        local prompt="Continue working on this task."
        [ -n "$feedback" ] && prompt="Previous attempt failed verification. Feedback: ${feedback}. Please fix and try again."
        echo "[ai-helpers] Resuming session ${session_id}" >&2
        $claude_cmd -p "$prompt" -r "$session_id" --tools "$tools" $extra_args
    else
        # Fresh start: pipe task file as prompt
        echo "[ai-helpers] Starting fresh session" >&2
        cat "$task_file" | $claude_cmd -p - --tools "$tools" $extra_args
    fi
}
"#;

const FOREMAN_GUIDE: &str = r#"# wf Foreman Guide — AI Agent 包工头操作手册

你是一个 AI Agent，角色是**包工头 (Foreman)**。你的工作是使用 `wf` 工具管理多个开发任务的全生命周期。

## 你的核心职责

1. 启动任务 (`wf start`)
2. 监控进度 (`wf list` / `wf status`)
3. 在人工决策点做出判断 (`wf done` / `wf reset --step`)
4. 处理失败 (分析原因 → 重试 / 人工介入 / 放弃)
5. 协调多任务并发

## 心智模型

wf 是一个**可恢复的协程**。每个 task 沿固定步骤序列前进，遇到无法自决的节点就 yield（暂停），等你来推动。

关键理解：
- **wf 不推送，你必须轮询。** 没有 callback 通知你状态变化。
- **状态 = replay(日志)**。 JSONL 日志是唯一真相源，没有额外的状态文件。
- **每个 task 独立。** 各自有自己的 JSONL，互不干扰，可以并发。

## 状态机

```
Pending → Running → Waiting    (等你 wf done)
                  → Completed  (全部步骤完成)
                  → Failed     (步骤失败/窗口丢失)
                  → Stopped    (你主动停止)
```

- `Waiting` 和 `Failed` 都可以被 `wf stop` 停止
- `Waiting`、`Failed` 和 `Stopped` 都可以被 `wf reset --step` 重试当前步
- 任何非 `Pending` 状态都可以被 `wf reset` 完全重置
- `wf start --reset` = `wf reset` + `wf start` 合并为一步

## Step 类型速查

| 类型 | 配置 | 行为 | 你需要做什么 |
|------|------|------|-------------|
| **普通 sync** | `run: "cmd"` | 同步执行，自动推进 | 无需干预 |
| **Gate** | 无 `run` | 立即 yield，等 approval | `wf done <task>` |
| **verify:human** | `verify: "human"` | 运行后等人工审核 | 审查输出 → `wf done` 或 `wf reset --step` |
| **verify:script** | `verify: "script.sh"` | 运行后自动验证 | 失败时看 on_fail 决定 |
| **on_fail:retry** | `on_fail: "retry"` | 失败自动重试(max N次) | 耗尽后变 Failed，你决定下一步 |
| **on_fail:human** | `on_fail: "human"` | 失败 yield 给你 | 分析反馈 → `wf done` 或 `wf reset --step` |
| **in_window** | `in_window: true` | 在 tmux 窗口执行，等 `wf done` | 监控窗口 → `wf done <task>` |

## 包工头主循环

```
while 有未完成的 task:
    1. wf list                          # 扫描全局状态
    2. 对每个 waiting 的 task:
       - 看 INFO 列判断等待原因
       - gate → 直接 wf done（如果准入条件满足）
       - needs review → wf capture/wf status 查看产物 → wf done 或 wf reset --step
       - needs decision → wf log --step N 看失败原因 → 修复后 wf reset --step 或 wf done
    3. 对每个 failed 的 task:
       - wf status --json 看 last_feedback
       - 可修复 → 修复环境 → wf reset --step
       - 不可修复 → wf reset 全部重来 或 wf stop 放弃
    4. 对每个 running + in_window 的 task:
       - wf capture 查看进展
       - 必要时 wf enter 进入窗口交互
    5. sleep/等待一段时间后重复
```

## 常用命令速查

### 日常操作
```bash
wf list                              # 全局状态一览 (关注 STATUS 和 INFO 列)
wf status <task> --json              # 单任务详情 (programmatic)
wf start <task>                      # 启动任务
wf start <task> --reset              # 重置并重新启动
wf done <task>                       # 放行 / 审核通过 / 标记完成
wf done <task> -m "reason"           # 附带消息的 done
wf stop <task>                       # 停止 (Running/Waiting 状态)
wf reset <task>                      # 完全重置到初始状态
wf reset --step <task>               # 只重试当前步骤
```

### 监控与诊断
```bash
wf capture <task>                    # 查看 tmux 窗口内容 (in_window 步骤)
wf capture <task> -l 100             # 查看最近 100 行
wf log <task> --all                  # 当前轮次的完整日志
wf log <task> --step 2               # 查看特定步骤 (0-based)
wf events <task>                     # 原始 JSONL 事件流
wf enter <task>                      # 附加到 tmux 窗口 (交互式)
```

### 等待与协调
```bash
wf wait <task> --until waiting -t 60       # 等到 waiting，超时 60 秒
wf wait <task> --until completed,failed    # 等到终态
```

## 关键决策场景

### 场景 1: Gate 步骤 (reason: gate)
Gate 是人工卡点，通常用于确认前置条件。
```bash
wf status <task>                     # 看当前步骤名，理解卡点意图
wf done <task>                       # 确认放行
```

### 场景 2: 人工审核 (reason: verify_human)
步骤执行成功了，但需要你审查产物。
```bash
wf log <task> --step <N>             # 看步骤输出
wf capture <task>                    # 如果是 in_window，看窗口内容
# 满意:
wf done <task>
# 不满意:
wf reset --step <task>               # 重跑这一步
```

### 场景 3: 失败等人工 (reason: on_fail_human)
步骤失败了，on_fail 设为 human，等你决策。
```bash
wf status <task> --json              # 看 last_feedback 了解失败原因
wf log <task> --step <N>             # 看完整输出
# 如果是环境问题，修复后:
wf reset --step <task>               # 重试
# 如果产物其实可以接受:
wf done <task>                       # 强制通过
# 如果无法修复:
wf stop <task>                       # 放弃这个 task
```

### 场景 4: Failed (retry 耗尽 / 无 on_fail)
步骤失败了且没有人工介入通道（或 retry 已耗尽）。
```bash
wf status <task> --json              # 看 retry_count 和 last_feedback
# 修复问题后:
wf reset --step <task>               # 重试当前步
# 或者从头来:
wf start <task> --reset
```

### 场景 5: in_window 步骤监控
in_window 步骤在 tmux 窗口中运行（通常是 AI agent 在里面开发）。
```bash
wf capture <task>                    # 非侵入式查看窗口内容
wf enter <task>                      # 直接进入窗口交互 (Ctrl-B d 退出)
# agent 完成工作后:
wf done <task> -m "development complete"
```

## 多任务管理

### 并发启动
```bash
wf start task-a &
wf start task-b &
wait                                 # 两个 task 并行推进直到各自遇到 yield 点
```

### 优先级判断
用 `wf list` 的输出来排优先级：
- `waiting` + `needs decision` → **最高**，有 task 被阻塞等你
- `waiting` + `gate` → **高**，检查准入条件后放行
- `waiting` + `needs review` → **高**，审查后放行
- `failed` → **中**，分析是否值得修复
- `running` → **低**，正常执行中，不需要干预

### 依赖管理
Task 可以有 `depends` 字段。被依赖的 task 必须先 Completed。
```bash
wf list                              # blocked task 的 INFO 列显示 "waiting: dep-task"
```

## 注意事项

### window_lost 是被动检测
tmux 窗口消失（进程崩溃、手动 kill）时，wf **不会主动通知你**。只有在你调用 `wf status`、`wf list`、`wf wait` 时才会检测到并标记为 Failed。所以：
- 对 in_window 步骤，定期 `wf list` 或 `wf capture` 检查健康
- 或者用 `wf wait <task> --until completed,failed,waiting` 阻塞等待状态变化

### Event Hook 是异步的
config 中的 `on` 字段定义的 hook 是 fire-and-forget。它们：
- 不保证执行顺序
- 失败不会影响主流程
- 可用于写日志、发通知，但不要依赖它们做决策

### 步骤索引
- **CLI 人类可读输出**: 1-based (`[1/8] setup`)
- **所有编程接口**: 0-based (`--step 0`、`--json` 输出、JSONL 事件、环境变量 `WF_STEP_INDEX`)
- 当你用 `wf log --step` 过滤时，用 0-based

### verify 脚本有环境变量
verify 脚本运行时可以访问所有 `WF_*` 环境变量：
- `WF_TASK` — 任务名
- `WF_STEP` — 当前步骤名
- `WF_STEP_INDEX` — 步骤索引 (0-based)
- `WF_REPO_ROOT` — 仓库根目录
- `WF_WORKTREE` — 工作树路径
- `WF_LOG_FILE` — JSONL 日志路径

### wf done 的双重语义
- 对 **Waiting** 状态: 等同于 approve，发 StepApproved 事件，步骤前进
- 对 **Running + in_window** 状态: 等同于标记完成，触发 verify 流程，可能继续或失败

### AI Worker 辅助
`.wf/lib/ai-helpers.sh` 提供了 AI worker 包装函数：
- `extract_session_id` — 从日志提取 session ID 用于续接
- `extract_feedback` — 提取失败反馈
- `run_ai_worker` — 自动判断新建/续接会话

## 诊断技巧

```bash
# 看一个 task 的完整事件历史 (调试用)
wf events <task> | jq .

# 看当前步骤的 retry 次数和最后反馈
wf status <task> --json | jq '{step: .current_step, retry: .retry_count, feedback: .last_feedback}'

# 看 event hook 是否正常触发 (如果配置了写文件的 hook)
tail -20 wf.log
```
"#;

pub fn run() -> Result<()> {
    // Get repo root
    let repo_root = get_repo_root()?;
    let wf_dir = Path::new(&repo_root).join(WF_DIR);

    // Check if already initialized
    if wf_dir.exists() {
        bail!(".wf/ directory already exists. Use 'wf reset' to reinitialize.");
    }

    println!("Initializing wf in {}...", repo_root);

    // Create directory structure
    fs::create_dir_all(wf_dir.join(TASKS_DIR))
        .context("Failed to create .wf/tasks/ directory")?;

    fs::create_dir_all(wf_dir.join(HOOKS_DIR))
        .context("Failed to create .wf/hooks/ directory")?;

    // Write default config
    let config_path = wf_dir.join(CONFIG_FILE);
    fs::write(&config_path, DEFAULT_CONFIG)
        .context("Failed to write config.jsonc")?;
    println!("  Created {}", config_path.display());

    fs::create_dir_all(wf_dir.join(LIB_DIR))
        .context("Failed to create .wf/lib/ directory")?;

    // Write hooks files
    create_hooks_files(&wf_dir, &repo_root)?;

    // Write lib files
    create_lib_files(&wf_dir)?;

    // Update .gitignore
    update_gitignore(&repo_root)?;

    println!("\nInitialization complete!");
    println!("\nNext steps:");
    println!("  1. Edit .wf/config.jsonc to customize your workflow");
    println!("  2. Create a task: wf create <name> [description]");
    println!("  3. Start the task: wf start <name>");

    Ok(())
}

fn create_hooks_files(wf_dir: &Path, repo_root: &str) -> Result<()> {
    let hooks_dir = wf_dir.join(HOOKS_DIR);

    // Write verify-stop.sh
    let verify_stop_path = hooks_dir.join("verify-stop.sh");
    fs::write(&verify_stop_path, VERIFY_STOP_SCRIPT)
        .context("Failed to write verify-stop.sh")?;

    // Set executable permission
    let mut perms = fs::metadata(&verify_stop_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&verify_stop_path, perms)
        .context("Failed to set verify-stop.sh permissions")?;
    println!("  Created {}", verify_stop_path.display());

    // Write settings.json with actual repo_root path
    let settings_content = SETTINGS_JSON_TEMPLATE.replace("REPO_ROOT", repo_root);
    let settings_path = hooks_dir.join("settings.json");
    fs::write(&settings_path, settings_content)
        .context("Failed to write settings.json")?;
    println!("  Created {}", settings_path.display());

    Ok(())
}

fn create_lib_files(wf_dir: &Path) -> Result<()> {
    let lib_dir = wf_dir.join(LIB_DIR);

    let helpers_path = lib_dir.join("ai-helpers.sh");
    fs::write(&helpers_path, AI_HELPERS_TEMPLATE)
        .context("Failed to write ai-helpers.sh")?;

    let mut perms = fs::metadata(&helpers_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&helpers_path, perms)
        .context("Failed to set ai-helpers.sh permissions")?;
    println!("  Created {}", helpers_path.display());

    // Write foreman guide
    let guide_path = lib_dir.join("foreman-guide.md");
    fs::write(&guide_path, FOREMAN_GUIDE)
        .context("Failed to write foreman-guide.md")?;
    println!("  Created {}", guide_path.display());

    Ok(())
}

fn update_gitignore(repo_root: &str) -> Result<()> {
    let gitignore_path = Path::new(repo_root).join(".gitignore");

    let current_content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path).unwrap_or_default()
    } else {
        String::new()
    };

    // Check if already has wf entries
    if current_content.contains(".wf/") {
        println!("  .gitignore already contains wf entries");
        return Ok(());
    }

    // Append wf entries
    let new_content = if current_content.is_empty() {
        GITIGNORE_ENTRIES.trim_start().to_string()
    } else if current_content.ends_with('\n') {
        format!("{}{}", current_content, GITIGNORE_ENTRIES)
    } else {
        format!("{}\n{}", current_content, GITIGNORE_ENTRIES)
    };

    fs::write(&gitignore_path, new_content)
        .context("Failed to update .gitignore")?;
    println!("  Updated .gitignore");

    Ok(())
}
