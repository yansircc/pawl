---
name: todo-py
---

# todo — CLI Todo Manager

命令行待办事项管理器。Python 写，JSON 文件存储。

## 命令

```
todo add "任务描述" [-p high/med/low]     # 添加待办
todo ls [--all] [--done]                   # 列出待办（默认只显示未完成）
todo done <id>                             # 标记完成
todo rm <id>                               # 删除
todo edit <id> "新描述"                    # 编辑
```

## 存储

JSON 文件，默认 `~/.todo.json`。

```json
[{"id": 1, "text": "...", "priority": "med", "done": false, "created": "ISO8601"}]
```

## 代码结构

```
todo/
├── todo.py      # 单文件，CLI + 存储逻辑
└── README.md
```

1 个文件，< 200 行。无外部依赖。

## 指示

我已经初始化了 wf。请帮我用 wf 配置这个项目的开发流程，需要读 PLAN.md 了解需求，然后配置 config 和创建 task。注意 claude_command 应设为 ccc。
