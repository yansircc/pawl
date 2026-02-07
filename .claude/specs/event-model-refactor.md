# Event Model Refactor — 设计文档

## 1. wf 是什么

一个**可恢复的协程**。沿固定序列前进，无法自决时让渡控制权。崩溃后从日志重建。

## 2. 生成规则（5 条）

1. **记忆 = 追加日志**。`state = replay(log)`，不独立存储。
2. **游标单调前进**。除显式 Reset 外，current_step 只增不减。
3. **裁决先于推进**。游标前进的前提是当前位置的成败已确定。
4. **两种权威**。每个决定点要么机器裁决（exit code），要么人类裁决（wf done），不会两者同时。
5. **失败可路由**。失败 → 重试（Reset）| 让渡（Yield）| 终止。

**唯一不变量**：`state = replay(log)`

## 3. 系统模型

### 权威三元（非二元）

| 权威 | 触发方式 | 代表事件 |
|------|---------|---------|
| 机器 | 进程退出（exit code） | StepCompleted |
| 人类 | 显式命令（wf done） | StepApproved |
| 环境 | 被动观测（health check） | WindowLost |

注：规则 4 说"两种权威"是对决定点的约束（一个步骤的成败由机器或人类裁决）。环境（WindowLost）是异常检测，不是裁决——它检测的是"执行环境崩溃"，不是"步骤成败"。

### 步骤属性矩阵

```
执行三元：sync / in_window / gate（无执行）
验证二元：cmd / human
失败路由二元：retry / human
```

step 的 4 个属性（run, verify, on_fail, in_window）参数化这个空间。非完全正交：gate 使 verify/on_fail 无意义。

### 事件的 4 个原语

10 个事件是 4 个原语的实例化：

| 原语 | 语义 | 实例 |
|---|---|---|
| **Advance** | 游标+1 | StepCompleted(0), StepApproved, StepSkipped |
| **Yield** | 让渡 | StepWaiting |
| **Fail** | 当前位置失败 | StepCompleted(≠0), WindowLost |
| **Reset** | 游标回退 | StepReset, TaskReset |

\+ 2 生命周期（TaskStarted, TaskStopped）+ 1 观测（WindowLaunched）= 10。最小集。

## 4. Phase 1：消除 VerifyFailed — ✅ 已完成

**核心改动**：StepCompleted 发射权从 3 个调用点收归 `handle_step_completion` 内部。

**签名**：
```rust
pub struct RunOutput {
    pub duration: Option<f64>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

pub fn handle_step_completion(
    project, task_name, step_idx, exit_code, step, run_output
) -> Result<bool>
```

**内部流程**：
```
exit_code != 0
  → emit StepCompleted(exit_code, run_output) → apply_on_fail

exit_code == 0, no verify
  → emit StepCompleted(0, run_output) → advance

exit_code == 0, verify:human
  → emit StepCompleted(0, run_output) + StepWaiting("verify_human")

exit_code == 0, verify:cmd passed
  → emit StepCompleted(0, run_output) → advance

exit_code == 0, verify:cmd failed
  → emit StepCompleted(1, stderr=feedback) → apply_on_fail
```

**效果**：
- 事件数 11 → 10（删除 VerifyFailed）
- run_verify 变为纯函数（无副作用），新增 HumanRequired 变体
- verify:command 路径恢复规则 3 合规
- 删除 config.rs 中已无用的 `verify_is_human()` 方法

**语义变迁**：verify:cmd 失败 → StepCompleted(exit_code=1, stderr=feedback)。run 的 stdout/stderr 有意丢弃，诊断来自 verify。

## 5. Phase 2/3 终审 — ❌ 不做

### Phase 2（verify:human 补偿）— 虚假问题

`StepCompleted(0) + StepWaiting(verify_human)` 不是补偿事务，是**正交状态叠加**：
- StepCompleted(0) 记录的是 "run 执行完毕且成功"——这是已发生的事实
- StepWaiting 标记 "人类审批进行中"——这是另一维度
- replay 中 `current_step = *step` 是"标记让渡点"，不是"回滚"

与 VerifyFailed 的本质区别：VerifyFailed 是同一维度上的判定翻转（run 成功→verify 否决），verify:human 是两个正交维度的叠加（run 结果 + 人类审批）。

崩溃瞬态（两个 append 之间崩溃导致 replay 认为步骤已成功）：窗口极小，实际无害。

### Phase 3（StepSkipped 合并）— 过度合并

"没执行"(Skipped) ≠ "执行成功"(Completed)。合并节省 1 个 enum variant，牺牲语义正交性。不值。

**结论：10 事件就是终态。**

## 6. 下一步重构：resolve/dispatch 分离

### 问题

当前 `handle_step_completion` 混合**决策**（exit_code + verify → Action）和**执行**（发射事件、执行命令）。

### 方案

```rust
// 纯函数：可穷举测试
pub fn resolve(
    exit_code: i32,
    verify_outcome: VerifyOutcome,
    on_fail: Option<&str>,
    retry_count: usize,
    max_retries: usize,
) -> Action

pub enum Action {
    Advance,                          // 步骤成功，前进
    YieldVerifyHuman,                 // 成功但需人工验证
    Retry { feedback: String },       // 失败，自动重试
    YieldOnFailHuman { feedback: String }, // 失败，等待人工
    Fail { feedback: String },        // 失败，不可恢复
}

// IO 层：只做副作用
fn dispatch(action, project, task_name, step_idx, run_output) -> Result<bool>
```

### 调用链

```
调用点 → pre_resolve(运行 verify, 统计 retry) → resolve(纯函数) → dispatch(IO)
```

3 个调用点（execute_step, done, on_exit）改动对称。

### 收益

- resolve() 的 7 条决策路径可被纯函数单元测试覆盖
- 决策逻辑与 IO 彻底分离
- 净代码量变化约 +10 行

### 陷阱

- StepCompleted 的 exit_code 映射：verify 失败时 exit_code=1（verify 的），非 run 的原始 exit_code
- verify 执行（run_verify）有 IO 副作用，必须在 resolve() 之前调用
- VerifyOutcome 和 run_verify 需从模块私有提升为 pub(crate)

## 7. 规则审计发现

三个独立 agent 并行审计（2025-02-07），11 项违反：

### 高严重度（3 项）

| ID | 规则 | 位置 | 描述 |
|----|------|------|------|
| V1 | 规则 1 | common.rs replay_task_with_health_check | 查询函数混入写副作用（append WindowLost） |
| V3 | 规则 1 | common.rs + wait.rs + control.rs | WindowLost 三重发射竞态，无互斥协调 |
| V7 | 规则 4 | control.rs on_exit + approve.rs done | in_window 步骤中 on_exit 和 wf done 双权威竞态 |

**根因收敛**：V1/V3/V7 指向同一架构缺陷——WindowLost 检测散布 3 处，无统一裁决者。

### 中严重度（4 项）

| ID | 规则 | 位置 | 描述 |
|----|------|------|------|
| V2 | 规则 1 | wait.rs append_window_lost | 绕过 Project::append_event()，event hook 不触发 |
| V4 | 规则 2 | event.rs + start.rs | verify:human 的 StepCompleted+StepWaiting 游标先进后退（已判定为可接受） |
| V5 | 规则 3 | start.rs HumanRequired 分支 | 两个 append 间崩溃瞬态（窗口极小，实际无害） |
| V8 | 规则 4 | wait.rs poll 循环 | wf wait 成为第三裁决者（可写入 WindowLost） |

### 低严重度（4 项）

| ID | 规则 | 位置 | 描述 |
|----|------|------|------|
| V6 | 规则 3 | start.rs verify fail→reset | StepCompleted 和 StepReset 非原子 |
| V9 | 规则 5 | start.rs apply_on_fail default | 默认终止路径无显式事件 |
| V10 | 规则 5 | start.rs retry 耗尽 | retry→终态转换无审计事件 |
| V11 | 规则 2 | event.rs replay StepReset | current_step 可设为任意值，无防御性校验 |

### 对偶发现

| 发现 | 说明 |
|------|------|
| gate 分类修正 | gate 不是验证维度的人类端，是执行维度的第三形态（无执行） |
| done 的不对称 | 人类只能通过 done 报告成功，失败走 reset --step。有意设计 |
| on_exit 丢失 RunOutput | in_window 进程退出路径无 stdout/stderr/duration 记录 |
| 缺少 timeout 维度 | sync 步骤可无限阻塞，in_window 有 health check 兜底 |

## 8. 重构优先级

| 优先级 | 改动 | 来源 | 收益 |
|--------|------|------|------|
| **P0** | resolve/dispatch 分离 | 视角 A | 可测试性飞跃，7 条纯函数测试 |
| **P1** | WindowLost 裁决统一 | V1/V3/V7/V8 | 消除 4 项规则违反，解决竞态 |
| **P2** | wf wait 写入走 Project API | V2 | 修复 event hook 失效 |
| 观察 | verify:human 崩溃瞬态 | V5 | 窗口极小，暂不处理 |
| 观察 | retry 耗尽审计事件 | V10 | 低频场景 |
| 未来 | timeout 维度 | 对偶发现 | 新功能，非重构 |

## 9. 对偶视角总结

| 对偶 | 说明 |
|------|------|
| Step / Event | 声明 vs 记录。replay 是翻译层。 |
| 决策 / 执行 | resolve（纯函数）vs dispatch（IO）。当前混合在 handle_step_completion 中。 |
| 机器 / 人类 | 同一步骤结构的两个投影，区别仅在授权来源。 |
| 同步 / 异步 | execute_step vs in_window。handle_step_completion 是汇合点。 |
| 主动 / 被动 | StepCompleted（进程退出）vs WindowLost（health check 发现）。不可合并。 |
| Gate / verify:human | 都产生 Yield→Resume 对。差异：gate 无执行产出，verify:human 有。 |
