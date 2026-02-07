use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::util::git::get_repo_root;

const WF_DIR: &str = ".wf";
const CONFIG_FILE: &str = "config.jsonc";
const TASKS_DIR: &str = "tasks";
const LIB_DIR: &str = "lib";
const SKILL_DIR: &str = ".claude/skills/wf";

const DEFAULT_CONFIG: &str = include_str!("templates/config.jsonc");

const GITIGNORE_ENTRIES: &str = r#"
# wf - Workflow Task Runner
.wf/*
!.wf/tasks/
!.wf/config.jsonc
!.wf/lib/
"#;

const AI_HELPERS_TEMPLATE: &str = include_str!("templates/ai-helpers.sh");

const WF_SKILL: &str = include_str!("templates/wf-skill.md");

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

    fs::create_dir_all(wf_dir.join(LIB_DIR))
        .context("Failed to create .wf/lib/ directory")?;

    // Write default config
    let config_path = wf_dir.join(CONFIG_FILE);
    fs::write(&config_path, DEFAULT_CONFIG)
        .context("Failed to write config.jsonc")?;
    println!("  Created {}", config_path.display());

    // Write lib files (only ai-helpers.sh)
    create_lib_files(&wf_dir)?;

    // Write skill files (.claude/skills/wf/)
    create_skill_files(&repo_root)?;

    // Update .gitignore
    update_gitignore(&repo_root)?;

    println!("\nInitialization complete!");
    println!("\nNext steps:");
    println!("  1. Edit .wf/config.jsonc to customize your workflow");
    println!("  2. Create a task: wf create <name> [description]");
    println!("  3. Start the task: wf start <name>");

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

    Ok(())
}

fn create_skill_files(repo_root: &str) -> Result<()> {
    let skill_dir = Path::new(repo_root).join(SKILL_DIR);
    fs::create_dir_all(&skill_dir)
        .context("Failed to create .claude/skills/wf/ directory")?;

    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, WF_SKILL)
        .context("Failed to write SKILL.md")?;
    println!("  Created {}", skill_path.display());

    let foreman_path = skill_dir.join("foreman-guide.md");
    fs::write(&foreman_path, FOREMAN_GUIDE)
        .context("Failed to write foreman-guide.md")?;
    println!("  Created {}", foreman_path.display());

    let task_guide_path = skill_dir.join("task-authoring-guide.md");
    fs::write(&task_guide_path, TASK_AUTHORING_GUIDE)
        .context("Failed to write task-authoring-guide.md")?;
    println!("  Created {}", task_guide_path.display());

    let worker_guide_path = skill_dir.join("ai-worker-guide.md");
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
