# Session Handoff

## 本次 Session 完成的工作

### Phase 1: 消除 VerifyFailed 事件 + 第一性原理审视

**代码改动**（事件 11→10，29/29 测试通过，零 warning）：

| 文件 | 改动 |
|------|------|
| `src/cmd/start.rs` | +RunOutput 结构体，handle_step_completion 内部发射 StepCompleted，run_verify 纯函数化（+HumanRequired），apply_on_fail 删 verify_is_human 分支 |
| `src/model/event.rs` | 删 VerifyFailed 枚举+所有 match 分支+replay+2旧测试，加2新测试 |
| `src/model/config.rs` | 删已无用的 verify_is_human() |
| `src/cmd/approve.rs` | 删 StepCompleted 发射，构造 RunOutput 传入 handle_step_completion |
| `src/cmd/control.rs` | 同上 |
| `src/cmd/status.rs` | 删 VerifyFailed match arm |
| `src/cmd/log.rs` | 删 VerifyFailed 打印分支 |
| `src/cmd/init.rs` | extract_feedback() 从 grep verify_failed 改为 grep step_completed + exit_code!=0 |

**第一性原理分析**（三 agent 并行审计，发现记录到 spec）：
- 系统本质 = 可恢复的协程，5 条生成规则，唯一不变量 state=replay(log)
- 权威三元（机器/人类/环境），事件 4 原语（Advance/Yield/Fail/Reset），10 事件=最小集
- Phase 2/3 终审：不做（verify:human 是正交状态叠加；StepSkipped 合并牺牲语义）
- 11 项规则违反审计（3高/4中/4低），核心缺陷=WindowLost 散布无互斥

---

## 待实现（下一 session 入口）

**读 `.claude/specs/event-model-refactor.md` 第 6/7/8 节**

| 优先级 | 改动 | 收益 |
|--------|------|------|
| **P0** | resolve/dispatch 分离 | handle_step_completion 拆为纯函数 resolve() + IO dispatch()，7 条路径可穷举测试 |
| **P1** | WindowLost 裁决统一 | 3 个发射点收归 1 处，消除 V1/V3/V7/V8 四项规则违反 |
| **P2** | wf wait 写入走 Project API | 修复 event hook 失效（V2） |

---

## 历史 Session

### Session 11: E2E 修复 6 个痛点
- P12 窗口检测、P4 Waiting reason、P3 wait health check、P9 last_feedback、错误信息、配置校验

### Session 9-10: 辩论驱动改进 + E2E Foreman 测试
- Step 0-based 统一、start --reset、events --follow、log 当前轮
- 8步 × 3 task × 12 phase 全自动测试

### Session 5-8: Foreman 模式 + P1-P5
- 非交互 Claude、wrapper.sh、事件 hook、并发 task、log --jsonl、wait 多状态

### Session 1-4: 架构演进
- TUI 删除 → Event Sourcing → Step 模型 → Unified Pipeline → E2E 测试

---

## 已知监控项

- **WindowLost 竞态**: 3 个发射点（common.rs/wait.rs/control.rs）无互斥，replay_with_health_check 查询混入写副作用
- **on_exit + wf done 双权威竞态**: in_window 步骤两个裁决者可同时触发
- **on_exit 丢失 RunOutput**: in_window 进程退出无 stdout/stderr/duration
- **retry 耗尽无审计事件**: 从 retry 转终态时无事件记录
- `extract_step_context()` 与 `count_auto_retries()` 逻辑相似但用途不同
- `wf events` 输出全部历史（不按当前轮过滤），与 `wf log --all` 不一致

## 关键文件索引

| 功能 | 文件 |
|------|------|
| CLI 定义（14 命令） | `src/cli.rs` |
| 配置模型（Step 4 属性） | `src/model/config.rs` |
| 事件模型 + replay（10 种） | `src/model/event.rs` |
| 状态投影类型 | `src/model/state.rs` |
| 任务定义（含 skip） | `src/model/task.rs` |
| 执行引擎 + 统一管线（RunOutput/handle_step_completion） | `src/cmd/start.rs` |
| 审批命令（done，统一管线） | `src/cmd/approve.rs` |
| 控制命令（stop/reset/on_exit + P12 窗口检查） | `src/cmd/control.rs` |
| 状态输出（retry_count/last_feedback） | `src/cmd/status.rs` |
| 日志输出（当前轮/全历史/--jsonl） | `src/cmd/log.rs` |
| 统一事件流（--follow） | `src/cmd/events.rs` |
| 等待命令（多状态 + health check） | `src/cmd/wait.rs` |
| 初始化（含 lib 模板） | `src/cmd/init.rs` |
| 公共工具（事件读写、钩子） | `src/cmd/common.rs` |
| 变量上下文 | `src/util/variable.rs` |
| 重构设计文档 | `.claude/specs/event-model-refactor.md` |
| 项目概述 | `.claude/CLAUDE.md` |
