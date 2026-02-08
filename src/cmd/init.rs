use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use crate::util::git::get_repo_root;

use super::common::PAWL_DIR;

const DEFAULT_CONFIG: &str = include_str!("templates/config.jsonc");
const PAWL_SKILL: &str = include_str!("templates/pawl-skill.md");

const GITIGNORE_ENTRIES: &str = r#"
# pawl - Resumable Step Sequencer
.pawl/*
!.pawl/tasks/
!.pawl/config.jsonc
!.pawl/skills/
"#;

pub fn run() -> Result<()> {
    let repo_root = get_repo_root()?;
    let pawl_dir = Path::new(&repo_root).join(PAWL_DIR);

    if pawl_dir.exists() {
        bail!(".pawl/ directory already exists. Use 'pawl reset' to reinitialize.");
    }

    println!("Initializing pawl in {}...", repo_root);

    fs::create_dir_all(pawl_dir.join("tasks"))
        .context("Failed to create .pawl/tasks/ directory")?;

    let config_path = pawl_dir.join("config.jsonc");
    fs::write(&config_path, DEFAULT_CONFIG)
        .context("Failed to write config.jsonc")?;
    println!("  Created {}", config_path.display());

    let skill_dir = pawl_dir.join("skills/pawl");
    fs::create_dir_all(&skill_dir)
        .context("Failed to create .pawl/skills/pawl/ directory")?;
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, PAWL_SKILL)
        .context("Failed to write SKILL.md")?;
    println!("  Created {}", skill_path.display());

    update_gitignore(&repo_root)?;

    println!("\nInitialization complete!");
    println!("\nNext steps:");
    println!("  1. Edit .pawl/config.jsonc to customize your workflow");
    println!("  2. Create a task: pawl create <name> [description]");
    println!("  3. Start the task: pawl start <name>");
    println!();
    println!("  To use with Claude Code:");
    println!("    mkdir -p .claude && mv .pawl/skills .claude/");
    println!("  To use with other AI tools:");
    println!("    Move .pawl/skills/* to your tool's skills directory");

    Ok(())
}

fn update_gitignore(repo_root: &str) -> Result<()> {
    let gitignore_path = Path::new(repo_root).join(".gitignore");

    let current_content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path).unwrap_or_default()
    } else {
        String::new()
    };

    if current_content.contains(".pawl/") {
        println!("  .gitignore already contains pawl entries");
        return Ok(());
    }

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
