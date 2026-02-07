# Session Handoff

## 本次 Session 完成的工作

### Session 17: Skill 精简 + Config 验证警告

**核心洞察**: Skill 文档的受众是 Claude（通过 skill 生成 config），不是人类。Claude 能从状态机推断决策逻辑、从 prompt 经验推断 task 写法——只需要提供 wf 特有的不可推断信息。

**Skill 精简** (4 文件 949 行 → 2 文件 ~200 行):
- `SKILL.md` 重写 ~130 行：+Config 设计规则(3条生成规则), +Claude CLI 集成模式(flag 组合), +ai-helpers.sh 函数表
- `reference.md` 新建 ~75 行：JSON schema, ai-helpers.sh 行为详解, hook 模式, 故障排查
- 删除 `foreman-guide.md` (决策逻辑可从状态机推断)
- 删除 `task-authoring-guide.md` (prompt 写作 Claude 已知)
- 删除 `ai-worker-guide.md` (不可推断部分已并入 SKILL.md)

**Config 模板强化**:
- develop 步骤加 verify + on_fail + `cd ${worktree}` (体现生成规则)
- review 步骤改为纯 gate (去掉无意义的 echo)
- event hook 默认写日志文件 (不再注释掉)

**Config 验证警告** (config.rs):
- in_window 无 verify → "wf done will assume success unconditionally"
- in_window run 不引用 worktree → "worker may execute in wrong directory"
- in_window 有 verify 无 on_fail → "verify failure is terminal"

**三条生成规则** (编码到 SKILL.md + config.rs):
1. 每个可失败的 in_window 步骤必须定义 on_fail
2. 每个有可观测产出的步骤必须定义 verify
3. in_window 步骤的 run 必须 cd ${worktree}

**技术指标**: 36 tests, zero warnings, 净减 ~1700 行

---

## 历史 Session

### Session 15-16: 文档体系 → Claude Code Skill
- 新增 3 份指南文档，重构为 `.claude/skills/wf/` skill 体系，config 模板精简

### Session 13-14: 重构 + E2E
- resolve/dispatch 分离、WindowLost 统一、wait.rs 走 Project API、E2E 包工头测试

### Session 9-12: 第一性原理 + 辩论驱动改进
- 事件模型审计、Step 0-based 统一、start --reset、events --follow

### Session 5-8: Foreman 模式
- 非交互 Claude、wrapper.sh、事件 hook、tmux 通知闭环

### Session 1-4: 架构演进
- TUI 删除 → Event Sourcing → Step 模型 → Unified Pipeline → E2E 测试

---

## 已知监控项

- **on_exit + wf done 双权威竞态**: in_window 步骤两个裁决者可同时触发
- **on_exit 丢失 RunOutput**: in_window 进程退出无 stdout/stderr/duration
- **retry 耗尽无审计事件**: 从 retry 转终态时无事件记录
- `wf events` 输出全部历史（不按当前轮过滤），与 `wf log --all` 不一致

## 关键文件索引

| 功能 | 文件 |
|------|------|
| CLI 定义（14 命令） | `src/cli.rs` |
| 配置模型 + **in_window 验证警告** | `src/model/config.rs` |
| 事件模型 + replay + count_auto_retries | `src/model/event.rs` |
| 执行引擎 + resolve/dispatch 管线 | `src/cmd/start.rs` |
| 初始化（生成 2 skill 文件） | `src/cmd/init.rs` |
| 模板文件 (config/skill/reference/ai-helpers) | `src/cmd/templates/` |
| 公共工具（事件读写、钩子、check_window_health） | `src/cmd/common.rs` |
| wf Skill 参考卡 + 生成规则 + Claude CLI | `.claude/skills/wf/SKILL.md` |
| 详细参考 (JSON schema/故障排查/hook) | `.claude/skills/wf/reference.md` |
| 项目概述 | `.claude/CLAUDE.md` |
