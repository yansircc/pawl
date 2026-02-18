use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::error::PawlError;

use super::common::PAWL_DIR;

const DEFAULT_WORKFLOW: &str = include_str!("templates/workflow.json");
const README: &str = include_str!("templates/readme.md");

const GITIGNORE_ENTRIES: &str = r#"
# pawl - Resumable Step Sequencer
.pawl/*
!.pawl/workflows/
"#;

pub fn run() -> Result<()> {
    // Use cwd as project root (.pawl/ doesn't exist yet, can't walk up)
    let project_root = std::env::current_dir()
        .context("Failed to get current directory")?
        .to_string_lossy()
        .to_string();
    let pawl_dir = Path::new(&project_root).join(PAWL_DIR);

    if pawl_dir.exists() {
        return Err(PawlError::AlreadyExists {
            message: ".pawl/ directory already exists. Use 'pawl reset' to reinitialize.".into(),
        }.into());
    }

    eprintln!("Initializing pawl in {}...", project_root);

    fs::create_dir_all(&pawl_dir)
        .context("Failed to create .pawl/ directory")?;

    let workflows_dir = pawl_dir.join("workflows");
    fs::create_dir_all(&workflows_dir)
        .context("Failed to create .pawl/workflows/ directory")?;

    let workflow_path = workflows_dir.join("default.json");
    fs::write(&workflow_path, DEFAULT_WORKFLOW)
        .context("Failed to write default.json")?;
    eprintln!("  Created {}", workflow_path.display());

    let readme_path = pawl_dir.join("README.md");
    fs::write(&readme_path, README)
        .context("Failed to write README.md")?;
    eprintln!("  Created {}", readme_path.display());

    update_gitignore(&project_root);

    eprintln!("\nInitialization complete!");
    eprintln!("\nNext steps:");
    eprintln!("  1. Edit .pawl/workflows/default.json to define your workflow");
    eprintln!("  2. Start a task: pawl start <name>");

    // Output JSON
    let json = serde_json::json!({
        "pawl_dir": pawl_dir.to_string_lossy(),
        "workflows_dir": workflows_dir.to_string_lossy(),
    });
    println!("{}", json);

    Ok(())
}

/// Best-effort .gitignore update (for git users)
fn update_gitignore(project_root: &str) {
    let gitignore_path = Path::new(project_root).join(".gitignore");

    let current_content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path).unwrap_or_default()
    } else {
        String::new()
    };

    if current_content.contains(".pawl/") {
        eprintln!("  .gitignore already contains pawl entries");
        return;
    }

    let new_content = if current_content.is_empty() {
        GITIGNORE_ENTRIES.trim_start().to_string()
    } else if current_content.ends_with('\n') {
        format!("{}{}", current_content, GITIGNORE_ENTRIES)
    } else {
        format!("{}\n{}", current_content, GITIGNORE_ENTRIES)
    };

    if let Ok(()) = fs::write(&gitignore_path, new_content) {
        eprintln!("  Updated .gitignore");
    }
}
