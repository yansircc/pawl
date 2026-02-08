use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use crate::util::git::get_repo_root;

use super::common::PAWL_DIR;

const DEFAULT_CONFIG: &str = include_str!("templates/config.jsonc");
const PAWL_SKILL: &str = include_str!("templates/pawl-skill.md");
const SKILL_AUTHOR: &str = include_str!("templates/author.md");
const SKILL_ORCHESTRATE: &str = include_str!("templates/orchestrate.md");
const SKILL_SUPERVISE: &str = include_str!("templates/supervise.md");

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

    eprintln!("Initializing pawl in {}...", repo_root);

    fs::create_dir_all(pawl_dir.join("tasks"))
        .context("Failed to create .pawl/tasks/ directory")?;

    let config_path = pawl_dir.join("config.jsonc");
    fs::write(&config_path, DEFAULT_CONFIG)
        .context("Failed to write config.jsonc")?;
    eprintln!("  Created {}", config_path.display());

    let skill_dir = pawl_dir.join("skills/pawl");
    let refs_dir = skill_dir.join("references");
    fs::create_dir_all(&refs_dir)
        .context("Failed to create .pawl/skills/pawl/references/ directory")?;
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, PAWL_SKILL)
        .context("Failed to write SKILL.md")?;
    eprintln!("  Created {}", skill_path.display());
    for (name, content) in [
        ("author.md", SKILL_AUTHOR),
        ("orchestrate.md", SKILL_ORCHESTRATE),
        ("supervise.md", SKILL_SUPERVISE),
    ] {
        let path = refs_dir.join(name);
        fs::write(&path, content).with_context(|| format!("Failed to write {}", name))?;
        eprintln!("  Created {}", path.display());
    }

    update_gitignore(&repo_root)?;

    eprintln!("\nInitialization complete!");
    eprintln!("\nNext steps:");
    eprintln!("  1. Edit .pawl/config.jsonc to customize your workflow");
    eprintln!("  2. Create a task: pawl create <name> [description]");
    eprintln!("  3. Start the task: pawl start <name>");
    eprintln!();
    eprintln!("  To use with Claude Code:");
    eprintln!("    mkdir -p .claude && mv .pawl/skills .claude/");
    eprintln!("  To use with other AI tools:");
    eprintln!("    Move .pawl/skills/* to your tool's skills directory");

    // Output JSON
    let json = serde_json::json!({
        "pawl_dir": pawl_dir.to_string_lossy(),
        "config": config_path.to_string_lossy(),
    });
    println!("{}", json);

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
        eprintln!("  .gitignore already contains pawl entries");
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
    eprintln!("  Updated .gitignore");

    Ok(())
}
