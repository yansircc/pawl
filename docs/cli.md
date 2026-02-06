# CLI Reference

> **v1 参考**：
> Clap CLI 定义 `/Users/yansir/code/tmp/worktree/src/cli.rs`
> 命令实现规范 `/Users/yansir/code/tmp/worktree/.claude/rules/cli/commands.md`

## 命令总览

| 命令 | 说明 | 用户 |
|------|------|------|
| `wf init` | 初始化项目 | 人 |
| `wf create` | 创建任务 | 人 |
| `wf list` | 列出所有任务 | 人 |
| `wf start` | 启动任务 | 人 |
| `wf status` | 查看状态 | 人 |
| `wf next` | 通过 checkpoint | 人 |
| `wf retry` | 重试当前 step | 人 |
| `wf back` | 回退到上一 step | 人 |
| `wf skip` | 跳过当前 step | 人 |
| `wf stop` | 停止当前进程 | 人 |
| `wf reset` | 重置任务 | 人 |
| `wf enter` | 进入任务窗口 | 人 |
| `wf log` | 查看 step 日志 | 人 |
| `wf done` | 标记 step 成功 | agent |
| `wf block` | 标记需要介入 | agent |
| `wf fail` | 标记 step 失败 | agent |

---

## 项目管理

### `wf init`

初始化项目。

```bash
wf init
```

行为：
1. 创建 `.wf/` 目录结构
2. 生成 `.wf/config.jsonc`（默认 workflow）
3. 创建 `.wf/tasks/` 目录
4. 添加 `.gitignore` 条目（worktrees, logs, status.json）

### `wf create <name> [description]`

创建任务。

```bash
wf create auth "实现用户认证"
wf create dashboard "实现仪表盘" --depends auth
```

| 参数 | 说明 |
|------|------|
| `name` | 任务名（必须是合法 git 分支名） |
| `description` | 任务描述（可选，省略则创建空模板） |
| `--depends <tasks>` | 逗号分隔的依赖列表 |

行为：
1. 创建 `.wf/tasks/{name}.md`
2. 写入 frontmatter（name, depends）和 description

### `wf list`

列出所有任务。

```bash
wf list
wf list --json
```

输出：
```
NAME        DEPENDS      STATUS
database    --           completed
auth        database     running [5/8]
dashboard   auth         pending
settings    --           running [5/8]
```

---

## 执行控制

### `wf start <task>`

启动任务，从 step 0 开始执行 workflow。

```bash
wf start auth
```

前置条件：
- 任务存在（`.wf/tasks/{task}.md`）
- 任务状态为 `pending`（未启动或已重置）
- 所有依赖已 `completed`

行为：
1. 检查依赖
2. 初始化状态（current_step = 0, status = running）
3. 开始执行 workflow
4. 遇到 checkpoint / in_window / 失败时暂停

### `wf next <task>`

通过 checkpoint，继续执行。

```bash
wf next auth
```

前置条件：
- 任务状态为 `waiting`（在 checkpoint 或 in_window step 完成后）

行为：
1. `current_step++`
2. 继续执行后续 steps

也支持 1-based 索引：
```bash
wf next 1    # 等同于 wf next <第一个任务>
```

### `wf retry <task>`

重新执行当前 step。

```bash
wf retry auth
```

前置条件：
- 任务状态为 `failed` 或 `stopped`

行为：
1. 保持 `current_step` 不变
2. 重新执行当前 step

### `wf back <task>`

回退到上一个 step。

```bash
wf back auth
```

前置条件：
- `current_step > 0`

行为：
1. `current_step--`
2. 状态设为 `waiting`（等待 `wf retry` 或 `wf next` 继续）

注意：MVP 不做 git 快照回退，只改 step index。用户需要自行处理代码状态。

### `wf skip <task>`

跳过当前 step，继续下一个。

```bash
wf skip auth
```

行为：
1. 当前 step 标记为 `skipped`
2. `current_step++`
3. 继续执行后续 steps

### `wf stop <task>`

停止任务的当前进程。

```bash
wf stop auth
```

行为：
1. 向任务的 tmux window 发送 Ctrl+C
2. 状态设为 `stopped`
3. 保留所有资源（window, worktree, branch）

### `wf reset <task>`

重置任务到初始状态。

```bash
wf reset auth
```

行为：
1. 停止进程（如果在运行）
2. 清理资源（window, worktree, branch）
3. 重置状态（current_step = 0, status = pending）

---

## 观测

### `wf status`

查看所有任务状态。

```bash
wf status           # 表格输出
wf status --json    # JSON 输出
```

表格输出：
```
NAME        STEP                  STATUS     TIME
auth        [5/8] Develop         running    15m
settings    [5/8] Develop         waiting    12m
dashboard   [3/8] Install deps    failed     5m
profile     --                    pending    (waiting: auth)
```

### `wf log <task>`

查看 step 日志。

```bash
wf log auth              # 当前 step 的日志
wf log auth --step 3     # 指定 step 的日志
wf log auth --all        # 所有 step 的日志
```

日志包含：
- 执行的命令
- stdout/stderr 输出
- exit code
- 耗时

### `wf enter <task>`

切换到任务的 tmux window。

```bash
wf enter auth
```

行为：
1. `tmux select-window -t {session}:{task}`

---

## Agent 命令

以下命令供 agent 在 tmux window 中调用。

### `wf done`

标记当前 step 成功。

> **v1 参考**：Agent 自验证机制确保 agent 在退出前调用此命令。
> Stop Hook 脚本 `/Users/yansir/code/tmp/worktree/src/commands/init/templates.rs:68-173`
> 验证清单 `/Users/yansir/code/tmp/worktree/src/commands/init/templates.rs:18-48`

```bash
wf done
```

行为：
1. 标记当前 step 为 `success`（append `AgentReported { result: Done }` 事件）
2. 继续执行后续 steps

环境变量 `WT_TASK` 和 `WT_STEP` 自动设置。

### `wf block [reason]`

标记需要人工介入。

```bash
wf block "数据库 schema 需要确认"
```

行为：
1. 标记当前 step 为 `blocked`（append `AgentReported { result: Blocked }` 事件）
2. 记录 reason
3. 任务状态变为 `waiting`

### `wf fail [reason]`

标记 step 失败。

```bash
wf fail "API 文档不完整"
```

行为：
1. 标记当前 step 为 `failed`（append `AgentReported { result: Failed }` 事件）
2. 记录 reason
3. 任务状态变为 `failed`

---

## 任务索引

所有接受 `<task>` 参数的命令都支持 1-based 索引：

```bash
wf start 1          # = wf start <第一个任务>
wf log 2             # = wf log <第二个任务>
wf enter 3           # = wf enter <第三个任务>
```

任务排序：按 `.wf/tasks/` 目录中的文件名排序。

## 任务名规则

任务名必须是合法的 git 分支名后缀：

- 不能为空
- 不能含空格、`~ ^ : ? * [ @ {`
- 不能以 `-` 或 `.` 开头
- 不能以 `.lock` 结尾
- 不能含 `..`
