use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::util::git::get_repo_root;

const PAWL_DIR: &str = ".pawl";
const CONFIG_FILE: &str = "config.jsonc";
const TASKS_DIR: &str = "tasks";
const LIB_DIR: &str = "lib";
const SKILL_DIR: &str = ".claude/skills/pawl";

const DEFAULT_CONFIG: &str = include_str!("templates/config.jsonc");

const GITIGNORE_ENTRIES: &str = r#"
# pawl - Resumable Step Sequencer
.pawl/*
!.pawl/tasks/
!.pawl/config.jsonc
!.pawl/lib/
.pawl/lib/node_modules/
"#;

const AI_HELPERS_TEMPLATE: &str = include_str!("templates/ai-helpers.sh");
const PLAN_WORKER_TEMPLATE: &str = include_str!("templates/plan-worker.mjs");
const PLAN_PACKAGE_TEMPLATE: &str = include_str!("templates/plan-package.json");

const PAWL_SKILL: &str = include_str!("templates/pawl-skill.md");

pub fn run() -> Result<()> {
    // Get repo root
    let repo_root = get_repo_root()?;
    let pawl_dir = Path::new(&repo_root).join(PAWL_DIR);

    // Check if already initialized
    if pawl_dir.exists() {
        bail!(".pawl/ directory already exists. Use 'pawl reset' to reinitialize.");
    }

    println!("Initializing pawl in {}...", repo_root);

    // Create directory structure
    fs::create_dir_all(pawl_dir.join(TASKS_DIR))
        .context("Failed to create .pawl/tasks/ directory")?;

    fs::create_dir_all(pawl_dir.join(LIB_DIR))
        .context("Failed to create .pawl/lib/ directory")?;

    // Write default config
    let config_path = pawl_dir.join(CONFIG_FILE);
    fs::write(&config_path, DEFAULT_CONFIG)
        .context("Failed to write config.jsonc")?;
    println!("  Created {}", config_path.display());

    // Write lib files
    create_lib_files(&pawl_dir)?;

    // Write skill files (.claude/skills/pawl/)
    create_skill_files(&repo_root)?;

    // Update .gitignore
    update_gitignore(&repo_root)?;

    println!("\nInitialization complete!");
    println!("\nNext steps:");
    println!("  1. Edit .pawl/config.jsonc to customize your workflow");
    println!("  2. Create a task: pawl create <name> [description]");
    println!("  3. Start the task: pawl start <name>");

    Ok(())
}

fn create_lib_files(pawl_dir: &Path) -> Result<()> {
    let lib_dir = pawl_dir.join(LIB_DIR);

    let helpers_path = lib_dir.join("ai-helpers.sh");
    fs::write(&helpers_path, AI_HELPERS_TEMPLATE)
        .context("Failed to write ai-helpers.sh")?;

    let mut perms = fs::metadata(&helpers_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&helpers_path, perms)
        .context("Failed to set ai-helpers.sh permissions")?;
    println!("  Created {}", helpers_path.display());

    let plan_worker_path = lib_dir.join("plan-worker.mjs");
    fs::write(&plan_worker_path, PLAN_WORKER_TEMPLATE)
        .context("Failed to write plan-worker.mjs")?;
    println!("  Created {}", plan_worker_path.display());

    let package_path = lib_dir.join("package.json");
    fs::write(&package_path, PLAN_PACKAGE_TEMPLATE)
        .context("Failed to write package.json")?;
    println!("  Created {}", package_path.display());

    Ok(())
}

fn create_skill_files(repo_root: &str) -> Result<()> {
    let skill_dir = Path::new(repo_root).join(SKILL_DIR);
    fs::create_dir_all(&skill_dir)
        .context("Failed to create .claude/skills/pawl/ directory")?;

    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, PAWL_SKILL)
        .context("Failed to write SKILL.md")?;
    println!("  Created {}", skill_path.display());

    Ok(())
}

fn update_gitignore(repo_root: &str) -> Result<()> {
    let gitignore_path = Path::new(repo_root).join(".gitignore");

    let current_content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path).unwrap_or_default()
    } else {
        String::new()
    };

    // Check if already has pawl entries
    if current_content.contains(".pawl/") {
        println!("  .gitignore already contains pawl entries");
        return Ok(());
    }

    // Append pawl entries
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
