# Execution Model

> **可复用代码参考**：完整的可复用模块索引见 [reusable-code.md](./reusable-code.md)

## 执行引擎

wf 的核心是一个顺序执行引擎。伪代码：

```rust
fn execute(task: &mut Task, workflow: &[Step], config: &Config) {
    while task.current_step < workflow.len() {
        let step = &workflow[task.current_step];

        match step.step_type() {
            // Checkpoint: 暂停等待人工
            Checkpoint => {
                // append_event(CheckpointReached) — 自动触发 on.checkpoint_reached hook
                task.status = Waiting;
                return;
            }

            // 普通 step: 同步执行
            Run { in_window: false } => {
                task.status = Running;
                let result = run_shell(&step.run, task, config);
                // append_event(CommandExecuted) — 自动触发 on.command_executed hook

                match result {
                    Ok(_) => task.current_step += 1,
                    Err(_) => {
                        task.status = Failed;
                        return;
                    }
                }
            }

            // in_window step: 发送到 window，等待状态标记
            Run { in_window: true } => {
                task.status = Running;
                // append_event(WindowLaunched) — 自动触发 on.window_launched hook
                send_to_window(&step.run, task, config);
                return;
            }
        }
    }

    // 所有 step 执行完毕 — replay() auto-derives Completed
    task.status = Completed;
}
```

## Step 执行详情

### 普通 Step

> **v1 参考**：Shell 命令执行封装 `/Users/yansir/code/tmp/worktree/src/services/command.rs`

```
wf start task
  → expand variables in step.run
  → spawn: sh -c "{expanded_command}"
  → capture stdout/stderr → .wf/logs/{task}/step-{N}-{name}.log
  → wait for exit
  → exit 0 → success → next step
  → exit 非0 → failed → stop
```

### in_window Step

> **v1 参考**：Tmux send-keys 封装 `/Users/yansir/code/tmp/worktree/src/services/multiplexer/tmux.rs`

```
wf start task
  → expand variables in step.run
  → wrap: "{expanded_command}; wf _on-exit $?"
  → tmux send-keys -t {session}:{window} "{wrapped_command}" Enter
  → task.status = running
  → return (不等待)

later:
  → agent 调用 wf done → task.current_step++ → continue execution
  → agent 调用 wf fail → task.status = failed
  → agent 调用 wf block → task.status = waiting
  → 进程退出未标记 → wf _on-exit 兜底处理
```

### Checkpoint

```
wf start task
  → 遇到 checkpoint
  → task.status = waiting
  → return

later:
  → 人工 wf next → task.current_step++ → continue execution
```

## wf _on-exit 兜底机制

> **v1 参考**：Agent 自验证机制使用 Claude CLI Stop Hook 实现类似功能。
> Stop Hook 脚本：`/Users/yansir/code/tmp/worktree/src/commands/init/templates.rs:68-173`
> Claude Settings 模板：`/Users/yansir/code/tmp/worktree/src/commands/init/templates.rs:50-65`
> 验证清单模板：`/Users/yansir/code/tmp/worktree/src/commands/init/templates.rs:18-48`
>
> Stop Hook 与 `_on-exit` 是互补的两层保障：
> - Stop Hook：在 agent（Claude CLI）退出前触发，提示 agent 自检并调用 `wf done`
> - `_on-exit`：在命令退出后触发，作为最终兜底

当 `in_window` 的进程退出但没有调用 `wf done/fail/block` 时：

```bash
# 发送到 window 的实际命令：
claude '@task.md'; wf _on-exit $?
```

`wf _on-exit` 的逻辑：

```rust
fn on_exit(exit_code: i32) {
    let task = load_current_task();  // 从 WT_TASK 环境变量

    // 如果已经标记了状态，不做任何事
    if task.status != Running {
        return;
    }

    // 根据 exit code 兜底标记
    if exit_code == 0 {
        mark_done(task);
    } else {
        mark_failed(task, format!("Process exited with code {}", exit_code));
    }
}
```

## 变量展开

> **v1 参考**：`/Users/yansir/code/tmp/worktree/src/services/executor/context.rs:114-173`

在执行 step.run 之前，所有 `${var}` 被替换：

```rust
fn expand(template: &str, task: &Task, config: &Config) -> String {
    let repo_root = git::get_repo_root();
    let worktree_dir = config.worktree_dir.unwrap_or(".wf/worktrees");

    template
        .replace("${task}", &task.name)
        .replace("${branch}", &format!("wf/{}", task.name))
        .replace("${worktree}", &format!("{}/{}/{}", repo_root, worktree_dir, task.name))
        .replace("${window}", &task.name)
        .replace("${session}", &config.session)
        .replace("${repo_root}", &repo_root)
        .replace("${step}", &current_step_name)
}
```

## Log 记录

每个 step 的 stdout/stderr 保存到文件：

```
.wf/logs/
  auth/
    step-0-create-branch.log
    step-1-create-worktree.log
    step-2-install-deps.log
    step-3-create-window.log
    step-4-develop.log           ← in_window 的 step: 记录发送的命令
    step-5-review-development.log ← checkpoint: 记录等待时间
    step-6-type-check.log
    step-7-lint.log
    step-8-merge.log
    step-9-cleanup.log
```

Log 文件格式：

```
=== Step 6: Type check ===
Command: cd /path/to/.wf/worktrees/auth && bun typecheck
Started: 2024-01-01T10:05:00Z

[stdout/stderr output here]

Exit code: 1
Duration: 3s
Status: failed
```

对于 `in_window` 的 step，log 只记录发送的命令和状态变化（实际输出在 tmux window 中可见）。

## Hook 执行

Hook 在 `append_event()` 中自动触发：写入 JSONL 后，检查 `config.on` 有无匹配 event type 的 hook，有则后台执行。Hook 失败只打印警告到 stderr，不影响 workflow。

## wf next 的完整流程

当用户执行 `wf next task`:

```
1. 加载 task 状态
2. 检查 task.status == waiting（在 checkpoint 或 in_window 完成后）
3. task.current_step++
4. 继续执行 workflow（调用 execute）
5. 保存状态
```

## wf done 的完整流程

当 agent 执行 `wf done`:

```
1. 从 WT_TASK 环境变量获取 task name
2. 加载 task 状态
3. 检查 task.status == running
4. 标记当前 step 为 success
5. task.current_step++
6. 继续执行后续 steps（调用 execute）
7. 保存状态
```

关键：`wf done` 不只是标记状态，还要**继续执行后续 steps**。这样 checkpoint 后面的同步 step 会自动执行。

## 并行隔离

多个任务并行执行，隔离通过以下方式保证：

| 维度 | 隔离方式 |
|------|---------|
| 代码 | 每个任务独立的 git worktree |
| 进程 | 每个任务独立的 tmux window |
| 状态 | status.json 中独立的 task entry |
| 日志 | 每个任务独立的 log 目录 |

状态文件的并发写入：使用文件锁（flock）确保原子性。
