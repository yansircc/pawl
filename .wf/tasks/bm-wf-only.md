---
name: bm-wf-only
---

# bm — CLI Bookmark Manager

命令行书签管理器。用 Go 写，SQLite 存储，支持标签和全文搜索。

## 命令

```
bm add <url> [-t tag1,tag2] [-n "note"]    # 添加书签，自动抓取 title
bm ls [-t tag] [--limit N]                  # 列出书签
bm search <keyword>                         # 全文搜索（url + title + note）
bm rm <id>                                  # 删除
bm tags                                     # 列出所有标签及数量
```

## 存储

SQLite 单文件，默认 `~/.bm.db`。

两张表：
- `bookmarks(id, url, title, note, created_at)`
- `tags(id, bookmark_id, name)`

## 依赖

- `modernc.org/sqlite` — 纯 Go SQLite
- `net/http` — 抓取页面 title（只读 `<title>` 标签，不需要 HTML parser，正则即可）

## 代码结构

```
bm/
├── main.go      # CLI 解析（os.Args，不用 flag 库）
├── db.go        # SQLite CRUD
├── fetch.go     # HTTP GET + 提取 title
└── go.mod
```

3 个文件，< 500 行。

## 指示

我已经初始化了 wf。请帮我用 wf 配置这个项目的开发流程，需要读 PLAN.md 了解需求，然后配置 config 和创建 task。注意 claude_command 应设为 ccc。
