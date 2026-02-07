# Task.md 编写指南 — 写出 AI Worker 能理解的任务文档

## 1. Task.md 的双重身份

每个 `.wf/tasks/{name}.md` 文件同时扮演两个角色：

**人类文档** — 描述任务需求、技术细节和验收标准，供 Foreman（包工头）理解任务意图。

**AI Worker 的 System Prompt** — 通过 `cat task.md | claude -p` 管道注入 Worker。Worker 看到的全部上下文就是这个文件的内容。

这意味着：
- 你写给人看的内容，AI 也会看到
- 不要假设 Worker "知道" 文件里没写的东西
- 任务描述的质量直接决定 Worker 的执行质量

## 2. YAML Frontmatter 完整字段

```yaml
---
name: fix-auth-bug
depends:
  - setup-database
  - deploy-config
skip:
  - setup
  - cleanup
---
```

### `name` (必须)

任务名，同时用作 git 分支名 `wf/{name}`。

命名规则：
- 必须是合法的 git branch name（无空格、无 `..`、无 `~^:`）
- 建议用小写 + 连字符：`fix-login-bug`、`add-search-api`
- 可以用斜杠分组：`feature/user-auth`、`bugfix/memory-leak`

### `depends` (可选)

依赖的其他 task 名称列表。被依赖的 task 必须先达到 `Completed` 状态，当前 task 才能启动。

两种写法均可：
```yaml
# 列表格式
depends:
  - database-setup
  - config-init

# 行内格式
depends: [database-setup, config-init]
```

### `skip` (可选)

要跳过的 workflow 步骤名列表。匹配 `config.jsonc` 中 step 的 `name` 字段。

```yaml
# 列表格式
skip:
  - setup
  - cleanup

# 行内格式
skip: [setup, cleanup]
```

被跳过的步骤会发出 `step_skipped` 事件，状态标记为 `Skipped`，cursor 直接前进。

## 3. 如何写好任务描述

任务描述是 Frontmatter 之后的 Markdown 正文。好的描述让 AI Worker 一次做对，差的描述导致反复迭代。

### 五要素

**目标陈述** — 一句话说清楚要做什么。

```markdown
## 目标
在 `src/api/auth.rs` 中添加 JWT token 刷新端点，支持 access_token 过期后用 refresh_token 换取新 token。
```

**技术约束** — 限定范围，避免 Worker 自由发挥。

```markdown
## 约束
- 使用 `jsonwebtoken` crate，不引入新依赖
- refresh_token 有效期 7 天，access_token 有效期 15 分钟
- 端点路径: POST /api/auth/refresh
- 复用现有的 `AuthError` 类型
```

**验收标准** — 可验证的，最好对应 verify 脚本能检查的条件。

```markdown
## 验收标准
- [ ] `cargo test` 全部通过
- [ ] `cargo clippy` 无 warning
- [ ] 新增端点有至少 3 个单元测试（正常刷新、过期 token、无效 token）
- [ ] 不修改现有 API 的行为
```

**代码风格** — 让产出物与现有代码一致。

```markdown
## 风格
- 遵循项目现有的 error handling 模式（`anyhow::Result`）
- 函数注释用 `///` 风格
- 测试放在同文件 `#[cfg(test)]` 模块中
```

**负面指导** — 明确说"不要做什么"比暗示更有效。

```markdown
## 不要做
- 不要重构现有的认证中间件
- 不要添加数据库迁移
- 不要修改 Cargo.toml
```

### 写作原则

- **具体 > 笼统**: "在 `handler.rs` 第 42 行后添加" 好于 "在合适的地方添加"
- **示例 > 描述**: 给一个期望的函数签名，比描述参数类型更清晰
- **一个文件一件事**: 如果任务涉及多个模块，按修改顺序列出每个文件要做什么

## 4. 迭代反馈模式

Task.md 不是写一次就定稿的。它是 Foreman 和 Worker 之间的通信协议，支持追加式迭代。

### 流程

```
1. Foreman 写初始 task.md
2. Worker 执行 → 失败（verify 不通过或 exit != 0）
3. Foreman 读失败信息:
   wf status <task> --json    # 看 last_feedback
   wf log <task> --step <N>   # 看完整输出
4. Foreman 在 task.md 末尾追加修复指导
5. wf reset --step <task>     # 重新执行当前步骤
6. Worker 读到更新后的 task.md → 第二轮尝试
```

### 追加格式

在文档末尾用分隔线 + 时间戳标记每轮反馈：

```markdown
---
## 修复指导 (Round 2)

上一轮问题: `cargo test` 失败，`test_refresh_expired_token` 断言错误。

原因: 你用了 `SystemTime::now()` 生成 token，测试中无法控制时间。

修复方案:
- 将 token 生成提取为 `fn create_token(claims, now: DateTime)`
- 测试中传入固定时间
- 不要修改其他测试
```

### 为什么追加而不是覆盖

- 保留修改历史，作为审计线索
- Worker 能看到之前的错误，避免重复犯错
- Foreman 能回顾决策过程

## 5. skip 字段的使用场景

### 已有 worktree 的热修复

workflow 的 `setup` 步骤通常创建 worktree。如果 worktree 已存在（之前的 task 留下的），跳过 setup 避免报错。

```yaml
skip: [setup]
```

### 需要保留环境的调试

开发调试时可能需要保留 worktree 和分支，跳过 cleanup。

```yaml
skip: [cleanup]
```

### 临时跳过耗时步骤

开发中临时跳过长时间运行的测试步骤：

```yaml
skip: [integration-test]
```

注意：skip 是 task 级别的，不影响其他 task 的同名步骤。

## 6. 完整示例

### 示例 1: 简单的代码修改任务

```markdown
---
name: fix-login-timeout
---

## 目标

修复登录接口超时问题。当前 `/api/login` 在数据库连接池耗尽时返回 500，应该返回 503 并带有 retry-after header。

## 修改范围

仅修改 `src/api/login.rs`:
- `handle_login()` 函数中捕获 `PoolTimeoutError`
- 返回 HTTP 503 + `Retry-After: 5` header

## 验收标准

- [ ] `cargo test` 通过
- [ ] 新增 `test_login_pool_timeout` 测试
- [ ] 不修改其他文件
```

### 示例 2: 有依赖的复杂任务（带 skip）

```markdown
---
name: add-search-api
depends:
  - setup-elasticsearch
skip:
  - setup
---

## 目标

在现有 API 服务中添加全文搜索端点，依赖已部署的 Elasticsearch 实例。

## 约束

- 依赖的 `setup-elasticsearch` task 已配置好 ES 连接和索引
- 复用 `setup-elasticsearch` 创建的 worktree（因此 skip setup）
- 使用 `elasticsearch-rs` crate（已在 Cargo.toml 中）

## 实现步骤

1. 在 `src/api/mod.rs` 注册 `/api/search` 路由
2. 创建 `src/api/search.rs`:
   - `SearchQuery { q: String, page: usize, size: usize }`
   - `SearchResult { items: Vec<Hit>, total: usize }`
   - `async fn handle_search(query: SearchQuery) -> Result<SearchResult>`
3. 在 `src/service/search.rs` 实现 ES 查询逻辑

## 验收标准

- [ ] `cargo build` 无 warning
- [ ] `cargo test` 通过（含新增的 search 测试）
- [ ] 搜索支持分页（page/size 参数）
- [ ] 空查询返回 400，非 500
```

### 示例 3: 包含迭代反馈历史的任务

```markdown
---
name: optimize-query
---

## 目标

优化 `get_user_orders()` 查询，当前在订单量 > 10000 时响应超过 5 秒。目标: P99 < 500ms。

## 约束

- 只优化查询，不改表结构
- 可以加索引（通过 migration）
- 不使用缓存方案

## 验收标准

- [ ] benchmark 测试 P99 < 500ms（10000 条数据）
- [ ] 现有测试全部通过

---

## 修复指导 (Round 2)

上一轮结果: benchmark P99 = 2.1s，改善不足。

分析: 你只加了单列索引 `idx_user_id`，但查询中有 `WHERE user_id = ? AND status IN (?) ORDER BY created_at DESC` 三个条件。

方案: 创建复合索引 `(user_id, status, created_at DESC)`，让查询走覆盖索引。
注意: 索引名用 `idx_orders_user_status_created`。

---

## 修复指导 (Round 3)

上一轮结果: benchmark P99 = 380ms，通过。但 `test_get_orders_empty_user` 失败。

原因: 你在优化中把 `COALESCE(count, 0)` 改成了 `count`，空用户返回 NULL 而非空列表。

修复: 恢复 COALESCE，只提交索引变更 + 查询优化，不动返回值处理逻辑。
```

## 7. 速查清单

写完 task.md 后，用这个清单自检：

- [ ] `name` 是合法的 git branch name
- [ ] 目标用一句话说清楚了
- [ ] 修改范围明确到文件级别
- [ ] 验收标准是可自动验证的（对应 verify 脚本）
- [ ] 写了"不要做什么"
- [ ] 没有假设 Worker 知道文件里没写的上下文