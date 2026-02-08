use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use crate::util::git::{get_repo_root, validate_branch_name};

use super::common::PAWL_DIR;
const TASKS_DIR: &str = "tasks";

pub fn run(name: &str, description: Option<&str>, depends: Option<&str>) -> Result<()> {
    // Validate task name
    validate_branch_name(name)?;

    // Get repo root and check .pawl exists
    let repo_root = get_repo_root()?;
    let pawl_dir = Path::new(&repo_root).join(PAWL_DIR);

    if !pawl_dir.exists() {
        bail!("Not a pawl project. Run 'pawl init' first.");
    }

    // Check if task already exists
    let task_path = pawl_dir.join(TASKS_DIR).join(format!("{}.md", name));
    if task_path.exists() {
        bail!("Task '{}' already exists at {}", name, task_path.display());
    }

    // Parse depends
    let depends_list: Vec<&str> = depends
        .map(|d| d.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    // Validate depends (check they are valid names, don't need to exist yet)
    for dep in &depends_list {
        validate_branch_name(dep)
            .with_context(|| format!("Invalid dependency name: {}", dep))?;
    }

    // Generate task content
    let content = generate_task_content(name, description, &depends_list);

    // Write task file
    fs::write(&task_path, content)
        .with_context(|| format!("Failed to write task file: {}", task_path.display()))?;

    println!("Created task: {}", task_path.display());

    if !depends_list.is_empty() {
        println!("  Dependencies: {}", depends_list.join(", "));
    }

    println!("\nNext: pawl start {}", name);

    Ok(())
}

fn generate_task_content(name: &str, description: Option<&str>, depends: &[&str]) -> String {
    let mut content = String::new();

    // Frontmatter
    content.push_str("---\n");
    content.push_str(&format!("name: {}\n", name));

    if !depends.is_empty() {
        content.push_str("depends:\n");
        for dep in depends {
            content.push_str(&format!("  - {}\n", dep));
        }
    }

    content.push_str("# skip:          # skip workflow steps not needed\n");
    content.push_str("#   - cleanup\n");
    content.push_str("---\n\n");

    // Body
    if let Some(desc) = description {
        content.push_str(&format!("## Task: {}\n\n", name));
        content.push_str(desc);
        content.push('\n');
    } else {
        content.push_str(&format!(
            "<!-- This file also serves as AI Worker system prompt (cat task.md | claude -p) -->\n\n\
             ## Task: {}\n\n\
             ### Goal\n\n\
             <!-- Clearly describe what to do -->\n\n\
             ### Constraints\n\n\
             <!-- Technical constraints, code standards, what NOT to do -->\n\n\
             ### Acceptance Criteria\n\n\
             - [ ] TODO\n",
            name
        ));
    }

    content
}
