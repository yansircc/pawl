# User Stories

## 1. 首次使用

**作为** 开发者
**我想** 快速初始化项目
**以便** 开始使用 wf 管理 AI 任务

```bash
cd my-project
wf init
# 创建 .wf/ 目录
# 生成 config.jsonc（含默认 workflow）
# 更新 .gitignore
```

编辑 `.wf/config.jsonc`，自定义 workflow。

---

## 2. 创建任务

**作为** 开发者
**我想** 快速定义一个开发任务
**以便** 让 agent 来完成

```bash
# 方式 1：手动创建
cat > .wf/tasks/auth.md << 'EOF'
---
name: auth
---
实现用户认证功能：
- 登录/注册 API
- JWT token 管理
- 中间件保护路由
EOF

# 方式 2：CLI 创建
wf create auth "实现用户认证功能"
```

---

## 3. 启动单个任务

**作为** 开发者
**我想** 一条命令启动任务
**以便** agent 开始在隔离环境中工作

```bash
wf start auth

# 输出：
# [1/6] Create branch ............ ok
# [2/6] Create worktree .......... ok
# [3/6] Install deps ............. ok (12s)
# [4/6] Create window ............ ok
# [5/6] Develop .................. running (in window 'auth')
# Task 'auth' is running. Use 'wf status' to monitor.
```

---

## 4. 启动多个并行任务

**作为** 开发者
**我想** 同时启动多个任务
**以便** 多个 agent 并行开发

```bash
wf start auth
wf start settings
wf start dashboard

wf status
# NAME        STEP              STATUS    TIME
# auth        [5/6] Develop     running   3m
# settings    [5/6] Develop     running   2m
# dashboard   [5/6] Develop     running   1m
```

---

## 5. 查看任务状态

**作为** 开发者
**我想** 一目了然看到所有任务进度
**以便** 知道哪些需要我介入

```bash
wf status

# NAME        STEP                  STATUS     TIME
# auth        [5/6] Develop         waiting    15m    ← wf done called
# settings    [5/6] Develop         running    12m
# dashboard   [3/6] Install deps    failed     5m     ← bun i failed
# profile     --                    pending           ← depends: auth
```

---

## 6. 查看失败日志

**作为** 开发者
**我想** 知道 step 为什么失败
**以便** 决定如何处理

```bash
wf log dashboard

# === Step 3: Install deps ===
# $ cd .wf/worktrees/dashboard && bun i
# error: Could not resolve "some-package"
# Exit code: 1
```

---

## 7. 处理失败的 step

**作为** 开发者
**我想** 在 step 失败后选择如何处理

```bash
# 选项 1：修复后重试
wf enter dashboard           # 进入 tmux window
# ... 手动修复 package.json ...
wf retry dashboard           # 重新执行当前 step

# 选项 2：跳过这个 step
wf skip dashboard

# 选项 3：重来
wf reset dashboard
wf start dashboard
```

---

## 8. 通过 checkpoint

**作为** 开发者
**我想** 在 agent 完成开发后审查代码
**以便** 确认质量后再继续

```bash
wf status
# auth    [6/8] 确认开发完成    waiting    20m

wf enter auth                 # 进入 window 查看代码
# ... 检查 agent 的实现 ...

wf next auth                  # 确认通过，继续后续 step
# [7/8] Type check .............. ok
# [8/8] Merge ................... ok
# Task 'auth' completed.
```

---

## 9. Agent 自主标记状态

**作为** AI agent
**我想** 标记自己的工作状态
**以便** workflow 能自动推进

```bash
# Agent 在 tmux window 中完成工作后：
wf done                       # 标记成功，workflow 继续

# Agent 遇到需要人工决策的问题：
wf block "数据库 schema 需要确认"    # 暂停，等待人工

# Agent 遇到无法解决的错误：
wf fail "API 文档不完整，无法实现"    # 标记失败
```

---

## 10. 依赖任务自动检查

**作为** 开发者
**我想** 任务的依赖自动检查
**以便** 不会在依赖未完成时启动任务

```bash
# profile 依赖 auth
wf start profile
# Error: Task 'profile' depends on 'auth' which is not completed (status: running)

# auth 完成后
wf start profile              # 正常启动
```

---

## 11. Hook 触发通知

**作为** 开发者
**我想** 任务完成时收到通知
**以便** 不用一直盯着屏幕

```jsonc
// config.jsonc
"on": {
  "command_executed": "terminal-notifier -title 'wf' -message '${task} completed'",
  "agent_reported": "terminal-notifier -title 'wf' -message '${task}: ${step} failed'"
}
```

```bash
# agent 完成任务 → typecheck 通过 → merge 完成
# → 系统通知弹出: "auth completed"
```

---

## 12. 回退到上一步

**作为** 开发者
**我想** 回退到上一个 step
**以便** 重新执行

```bash
wf status
# auth    [7/8] Type check    failed

wf back auth                  # 回退到 step 6
# auth    [6/8] 确认开发完成   waiting

wf enter auth                 # 进去让 agent 修 typecheck 问题
# ... "请修复 typecheck 错误" ...

wf next auth                  # 继续
# [7/8] Type check .............. ok
# [8/8] Merge ................... ok
```

---

## 13. 完整的项目开发流程

```bash
# 1. 初始化
wf init

# 2. 拆分任务
wf create database "设计数据库 schema"
wf create auth "实现用户认证" --depends database
wf create dashboard "实现仪表盘" --depends auth
wf create settings "实现设置页面"

# 3. 查看任务
wf list
# NAME        DEPENDS      STATUS
# database    --           pending
# auth        database     pending
# dashboard   auth         pending
# settings    --           pending

# 4. 启动无依赖的任务
wf start database
wf start settings

# 5. 监控
wf status
# NAME        STEP              STATUS     TIME
# database    [5/8] Develop     running    5m
# settings    [5/8] Develop     running    3m
# auth        --                pending    (waiting: database)
# dashboard   --                pending    (waiting: auth)

# 6. database 完成 → 启动 auth
wf next database              # 通过 checkpoint
# ... workflow 继续直到 completed ...
wf start auth                 # 依赖满足，可以启动

# 7. 最终所有任务完成
wf status
# NAME        STEP    STATUS       TIME
# database    --      completed    15m
# auth        --      completed    20m
# dashboard   --      completed    12m
# settings    --      completed    18m
```

---

## 14. 停止和恢复

**作为** 开发者
**我想** 暂停和恢复任务
**以便** 在需要时控制资源使用

```bash
# 暂停任务（发送 Ctrl+C 到 window）
wf stop auth
# Sent Ctrl+C to window 'auth'
# Task 'auth' stopped at step 5.

# 查看状态
wf status
# auth    [5/8] Develop    stopped

# 恢复执行（重新执行当前 step）
wf retry auth
# [5/8] Develop .............. running (in window 'auth')
```

---

## 15. 清理和重置

```bash
# 重置单个任务（清理所有资源，回到 step 0）
wf reset auth
# Cleaned up: window 'auth', worktree, branch
# Task 'auth' reset to step 0.

# 重新开始
wf start auth
```
