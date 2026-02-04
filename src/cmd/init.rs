use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use crate::util::git::get_repo_root;

const WF_DIR: &str = ".wf";
const CONFIG_FILE: &str = "config.jsonc";
const TASKS_DIR: &str = "tasks";

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

  // ============================================
  // Workflow
  // ============================================
  // 所有任务共享的执行流程
  // 支持变量: ${task}, ${branch}, ${worktree}, ${window},
  //          ${session}, ${repo_root}, ${step}

  "workflow": [
    // 创建资源
    { "name": "Create branch", "run": "git branch ${branch}" },
    { "name": "Create worktree", "run": "git worktree add ${worktree} ${branch}" },
    { "name": "Create window", "run": "tmux new-window -t ${session} -n ${window} -c ${worktree}" },

    // 开发（在 tmux window 中执行）
    {
      "name": "Develop",
      "run": "claude -p '@.wf/tasks/${task}.md'",
      "in_window": true
    },

    // 人工确认开发完成（checkpoint）
    { "name": "Review development" },

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
  // Hooks（可选）
  // ============================================
  // 事件触发的 shell 命令，fire-and-forget
  //
  // "hooks": {
  //   "task.completed": "terminal-notifier -title 'wf' -message '${task} completed'",
  //   "step.failed": "terminal-notifier -title 'wf' -message '${task}: ${step} failed'"
  // }
}
"#;

const GITIGNORE_ENTRIES: &str = r#"
# wf - Workflow Task Runner
.wf/*
!.wf/tasks/
!.wf/config.jsonc
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

    // Write default config
    let config_path = wf_dir.join(CONFIG_FILE);
    fs::write(&config_path, DEFAULT_CONFIG)
        .context("Failed to write config.jsonc")?;
    println!("  Created {}", config_path.display());

    // Update .gitignore
    update_gitignore(&repo_root)?;

    println!("\nInitialization complete!");
    println!("\nNext steps:");
    println!("  1. Edit .wf/config.jsonc to customize your workflow");
    println!("  2. Create a task: wf create <name> [description]");
    println!("  3. Start the task: wf start <name>");

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
