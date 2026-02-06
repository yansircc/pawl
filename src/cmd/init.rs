use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::util::git::get_repo_root;

const WF_DIR: &str = ".wf";
const CONFIG_FILE: &str = "config.jsonc";
const TASKS_DIR: &str = "tasks";
const HOOKS_DIR: &str = "hooks";

const DEFAULT_CONFIG: &str = r#"{
  // ============================================
  // 基础配置
  // ============================================

  // tmux session 名称（默认: 项目目录名）
  // "session": "my-project",

  // Terminal multiplexer（默认: "tmux"）
  // "multiplexer": "tmux",

  // Worktree 存放目录（相对于 repo root，默认: ".wf/worktrees"）
  // "worktree_dir": ".wf/worktrees",

  // 基础分支（用于创建任务分支的起点，默认: "main"）
  // "base_branch": "main",

  // ============================================
  // Workflow
  // ============================================
  // 所有任务共享的执行流程
  // 支持变量: ${task}, ${branch}, ${worktree}, ${window},
  //          ${session}, ${repo_root}, ${step}, ${base_branch}

  "workflow": [
    // 创建资源
    { "name": "Create branch", "run": "git branch ${branch} ${base_branch}" },
    { "name": "Create worktree", "run": "git worktree add ${worktree} ${branch}" },
    { "name": "Create window", "run": "tmux new-window -t ${session} -n ${window} -c ${worktree}" },

    // 开发（在 tmux window 中执行）
    {
      "name": "Develop",
      "run": "claude --settings ${repo_root}/.wf/hooks/settings.json -p '@.wf/tasks/${task}.md'",
      "in_window": true
    },

    // 人工确认开发完成
    { "name": "Review development", "verify": "human" },

    // 合并到主分支
    {
      "name": "Merge",
      "run": "cd ${repo_root} && git merge --squash ${branch} && git commit -m 'feat(${task}): merge from wf'"
    },

    // 清理资源
    {
      "name": "Cleanup",
      "run": "tmux kill-window -t ${session}:${window} 2>/dev/null; git -C ${repo_root} worktree remove ${worktree} --force 2>/dev/null; git -C ${repo_root} branch -D ${branch} 2>/dev/null; true"
    }
  ]

  // ============================================
  // Event Hooks（可选）
  // ============================================
  // 事件触发的 shell 命令，fire-and-forget
  // key 为 Event 类型名（snake_case），自动在 append_event 时触发
  //
  // "on": {
  //   "task_started": "echo '${task} started'",
  //   "command_executed": "echo '${task} step ${step} exit=${exit_code}'",
  //   "agent_reported": "echo '${task} agent: ${result} ${message}'",
  //   "window_lost": "echo '${task} window crashed at step ${step}'"
  // }
}
"#;

const VERIFY_STOP_SCRIPT: &str = r#"#!/bin/bash
# wf verify script - auto generated
INPUT=$(cat)
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
TRANSCRIPT=$(echo "$INPUT" | jq -r '.transcript_path // empty')

[ -z "$TRANSCRIPT" ] || [ ! -f "$TRANSCRIPT" ] && exit 0

STATE_FILE="/tmp/wf-verify-${SESSION_ID}.state"
LAST_LINE=0; [ -f "$STATE_FILE" ] && LAST_LINE=$(cat "$STATE_FILE")
CURRENT_LINE=$(wc -l < "$TRANSCRIPT" | tr -d ' ')
[ "$CURRENT_LINE" -le "$LAST_LINE" ] && exit 0

tail -n +$((LAST_LINE + 1)) "$TRANSCRIPT" | grep -q 'wf done\|wf fail' && { rm -f "$STATE_FILE"; exit 0; }

echo "$CURRENT_LINE" > "$STATE_FILE"
TASK="${WF_TASK:-task}"
echo "{\"decision\":\"block\",\"reason\":\"【自检】请确认任务完成后执行：\\n- 成功: wf done ${TASK}\\n- 失败: wf fail ${TASK} -m 原因\"}"
exit 0
"#;

const SETTINGS_JSON_TEMPLATE: &str = r#"{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "REPO_ROOT/.wf/hooks/verify-stop.sh"
          }
        ]
      }
    ]
  }
}
"#;

const GITIGNORE_ENTRIES: &str = r#"
# wf - Workflow Task Runner
.wf/*
!.wf/tasks/
!.wf/config.jsonc
!.wf/hooks/
"#;

pub fn run() -> Result<()> {
    // Get repo root
    let repo_root = get_repo_root()?;
    let wf_dir = Path::new(&repo_root).join(WF_DIR);

    // Check if already initialized
    if wf_dir.exists() {
        bail!(".wf/ directory already exists. Use 'wf reset' to reinitialize.");
    }

    println!("Initializing wf in {}...", repo_root);

    // Create directory structure
    fs::create_dir_all(wf_dir.join(TASKS_DIR))
        .context("Failed to create .wf/tasks/ directory")?;

    fs::create_dir_all(wf_dir.join(HOOKS_DIR))
        .context("Failed to create .wf/hooks/ directory")?;

    // Write default config
    let config_path = wf_dir.join(CONFIG_FILE);
    fs::write(&config_path, DEFAULT_CONFIG)
        .context("Failed to write config.jsonc")?;
    println!("  Created {}", config_path.display());

    // Write hooks files
    create_hooks_files(&wf_dir, &repo_root)?;

    // Update .gitignore
    update_gitignore(&repo_root)?;

    println!("\nInitialization complete!");
    println!("\nNext steps:");
    println!("  1. Edit .wf/config.jsonc to customize your workflow");
    println!("  2. Create a task: wf create <name> [description]");
    println!("  3. Start the task: wf start <name>");

    Ok(())
}

fn create_hooks_files(wf_dir: &Path, repo_root: &str) -> Result<()> {
    let hooks_dir = wf_dir.join(HOOKS_DIR);

    // Write verify-stop.sh
    let verify_stop_path = hooks_dir.join("verify-stop.sh");
    fs::write(&verify_stop_path, VERIFY_STOP_SCRIPT)
        .context("Failed to write verify-stop.sh")?;

    // Set executable permission
    let mut perms = fs::metadata(&verify_stop_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&verify_stop_path, perms)
        .context("Failed to set verify-stop.sh permissions")?;
    println!("  Created {}", verify_stop_path.display());

    // Write settings.json with actual repo_root path
    let settings_content = SETTINGS_JSON_TEMPLATE.replace("REPO_ROOT", repo_root);
    let settings_path = hooks_dir.join("settings.json");
    fs::write(&settings_path, settings_content)
        .context("Failed to write settings.json")?;
    println!("  Created {}", settings_path.display());

    Ok(())
}

fn update_gitignore(repo_root: &str) -> Result<()> {
    let gitignore_path = Path::new(repo_root).join(".gitignore");

    let current_content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path).unwrap_or_default()
    } else {
        String::new()
    };

    // Check if already has wf entries
    if current_content.contains(".wf/") {
        println!("  .gitignore already contains wf entries");
        return Ok(());
    }

    // Append wf entries
    let new_content = if current_content.is_empty() {
        GITIGNORE_ENTRIES.trim_start().to_string()
    } else if current_content.ends_with('\n') {
        format!("{}{}", current_content, GITIGNORE_ENTRIES)
    } else {
        format!("{}\n{}", current_content, GITIGNORE_ENTRIES)
    };

    fs::write(&gitignore_path, new_content)
        .context("Failed to update .gitignore")?;
    println!("  Updated .gitignore");

    Ok(())
}
