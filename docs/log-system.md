# 日志系统设计

## 设计理念

**职责分离**：

| 数据源 | 职责 |
|--------|------|
| `config.jsonc` | step 定义（name、command、type） |
| `status.json` | 运行时状态（current_step、step_status） |
| `{task}.jsonl` | **只存输出**（stdout/stderr 或 transcript 指针） |

日志不重复存储元数据，只记录每个 step 产生的输出。

## 文件结构

```
.wf/logs/
└── {task}.jsonl          # 单文件，JSONL 格式
```

## 日志格式

每个 step 完成后追加一行 JSON：

### 普通 step

```json
{"step":0,"exit_code":0,"duration":5.2,"stdout":"bun install v1.0.0\n...","stderr":""}
```

### in_window step (agent)

```json
{"step":1,"session_id":"8322e722-9b7b-406d-bee4-88170b8c676b","transcript":"/Users/xxx/.claude/projects/-xxx/8322e722-xxx.jsonl"}
```

### checkpoint step

```json
{"step":2}
```

## 获取 session_id

### 方式 1：-p 模式 + JSON 输出

```bash
OUTPUT=$(claude -p --output-format=json "prompt" 2>&1)
SESSION_ID=$(echo "$OUTPUT" | jq -r '.session_id')
```

### 方式 2：交互模式 + tmux capture

Claude 退出时会输出：
```
Resume this session with:
claude --resume 8322e722-9b7b-406d-bee4-88170b8c676b
```

提取命令：
```bash
SESSION_ID=$(tmux capture-pane -t {window} -p -S -100 | grep "claude --resume" | awk '{print $3}')
```

## Transcript 路径公式

```
~/.claude/projects/{path-hash}/{session_id}.jsonl
```

其中 `path-hash` = 工作目录绝对路径中的 `/` 替换为 `-`

示例：
- 工作目录：`/Users/yansir/code/project`
- path-hash：`-Users-yansir-code-project`

## 链式读取（核心用法）

Judge step 读取 Develop step 的输出：

```bash
# 1. 获取前一个 step 的日志行
PREV=$(tail -1 .wf/logs/${task}.jsonl)

# 2. 提取 transcript 路径
TX=$(echo "$PREV" | jq -r '.transcript')

# 3. 从 transcript 提取 Claude 的最终回复
RESULT=$(jq -s 'map(select(.type=="assistant")) | last | .message.content[0].text' "$TX")

# 4. 传给 Judge
claude -p "Review this output: $RESULT"
```

## 常用 jq 命令

```bash
# 查看某个 step 的日志
jq -s '.[] | select(.step==2)' .wf/logs/task.jsonl

# 获取最后一个 step 的 transcript
tail -1 .wf/logs/task.jsonl | jq -r '.transcript'

# 从 transcript 提取 Claude 回复
jq -s 'map(select(.type=="assistant")) | last | .message.content[0].text' /path/to/transcript.jsonl

# 从 transcript 提取所有工具调用
jq -s '[.[] | select(.type=="assistant") | .message.content[] | select(.type=="tool_use")]' /path/to/transcript.jsonl

# 统计所有 step 耗时
jq -s '[.[].duration | select(. != null)] | add' .wf/logs/task.jsonl
```

## 在 config.jsonc 中的使用

```jsonc
{
  "workflow": [
    {
      "name": "Develop",
      "in_window": true,
      "run": "claude -p '${task_content}'"
    },
    {
      "name": "Judge",
      "in_window": true,
      "run": "PREV_TX=$(tail -1 ${log_dir}/${task}.jsonl | jq -r '.transcript') && claude -p \"Review the code: $(jq -s 'map(select(.type==\"assistant\")) | last | .message.content[0].text' $PREV_TX)\""
    }
  ]
}
```

## 数据流示意

```
Develop step 完成
        │
        ▼
task.jsonl 追加一行: {"step":0,"session_id":"xxx","transcript":"/path/to/xxx.jsonl"}
        │
        ▼
Judge step 启动
        │
        ▼
tail -1 task.jsonl | jq '.transcript'  →  获取 transcript 路径
        │
        ▼
jq 'select(.type=="assistant")' transcript.jsonl  →  提取 Claude 回复
        │
        ▼
传给 Judge Claude 进行评审
```

## 优点

1. **单文件**：一个任务一个 JSONL 文件
2. **追加友好**：每个 step 完成追加一行，无需重写
3. **无数据重复**：不存储 config 中已有的元数据
4. **格式统一**：wf 日志是 JSONL，Claude transcript 也是 JSONL，一个 `jq` 走天下
5. **链式读取**：后续 step 可以方便地读取前序 step 的输出
