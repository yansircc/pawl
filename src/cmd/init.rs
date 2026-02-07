use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::util::git::get_repo_root;

const WF_DIR: &str = ".wf";
const CONFIG_FILE: &str = "config.jsonc";
const TASKS_DIR: &str = "tasks";
const HOOKS_DIR: &str = "hooks";
const LIB_DIR: &str = "lib";

const DEFAULT_CONFIG: &str = include_str!("templates/config.jsonc");

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

tail -n +$((LAST_LINE + 1)) "$TRANSCRIPT" | grep -q 'wf done' && { rm -f "$STATE_FILE"; exit 0; }

echo "$CURRENT_LINE" > "$STATE_FILE"
TASK="${WF_TASK:-task}"
echo "{\"decision\":\"block\",\"reason\":\"【自检】请确认任务完成后执行 wf done ${TASK}\"}"
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
!.wf/lib/
"#;

const AI_HELPERS_TEMPLATE: &str = include_str!("templates/ai-helpers.sh");

const FOREMAN_GUIDE: &str = include_str!("templates/foreman-guide.md");

const TASK_AUTHORING_GUIDE: &str = include_str!("templates/task-authoring-guide.md");

const AI_WORKER_GUIDE: &str = include_str!("templates/ai-worker-guide.md");

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

    fs::create_dir_all(wf_dir.join(LIB_DIR))
        .context("Failed to create .wf/lib/ directory")?;

    // Write hooks files
    create_hooks_files(&wf_dir, &repo_root)?;

    // Write lib files
    create_lib_files(&wf_dir)?;

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

fn create_lib_files(wf_dir: &Path) -> Result<()> {
    let lib_dir = wf_dir.join(LIB_DIR);

    let helpers_path = lib_dir.join("ai-helpers.sh");
    fs::write(&helpers_path, AI_HELPERS_TEMPLATE)
        .context("Failed to write ai-helpers.sh")?;

    let mut perms = fs::metadata(&helpers_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&helpers_path, perms)
        .context("Failed to set ai-helpers.sh permissions")?;
    println!("  Created {}", helpers_path.display());

    // Write foreman guide
    let guide_path = lib_dir.join("foreman-guide.md");
    fs::write(&guide_path, FOREMAN_GUIDE)
        .context("Failed to write foreman-guide.md")?;
    println!("  Created {}", guide_path.display());

    // Write task authoring guide
    let task_guide_path = lib_dir.join("task-authoring-guide.md");
    fs::write(&task_guide_path, TASK_AUTHORING_GUIDE)
        .context("Failed to write task-authoring-guide.md")?;
    println!("  Created {}", task_guide_path.display());

    // Write AI worker guide
    let worker_guide_path = lib_dir.join("ai-worker-guide.md");
    fs::write(&worker_guide_path, AI_WORKER_GUIDE)
        .context("Failed to write ai-worker-guide.md")?;
    println!("  Created {}", worker_guide_path.display());

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
