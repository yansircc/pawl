---
name: handoff
description: 执行 Session Handoff，为下一个 session 做好交接准备
---

执行 Session Handoff，为下一个 session 做好交接准备：

1. **整理 specs**
   - 检查 `.claude/specs/` 中的文件
   - 删除已实现的 spec
   - 将未实现的内容整理到一个新 spec 文件（如有）

2. **更新文档**
   - README.md - 确保命令列表、配置说明是最新的
   - .claude/CLAUDE.md - 更新项目概述、目录结构
   - .claude/skills/ - 更新相关 skill 文档

3. **更新 .claude/HANDOFF.md**
   - 记录本次 session 完成的工作
   - 精简之前 session 完成的工作
   - 列出待实现功能（如有）
   - 更新已知问题和相关文件索引

4. **创建 commit**
   - 将所有变更提交
   - 使用清晰的 commit message

不要 push 到远程。
