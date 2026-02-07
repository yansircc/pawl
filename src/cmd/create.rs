use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use crate::util::git::{get_repo_root, validate_branch_name};

const WF_DIR: &str = ".wf";
const TASKS_DIR: &str = "tasks";

pub fn run(name: &str, description: Option<&str>, depends: Option<&str>) -> Result<()> {
    // Validate task name
    validate_branch_name(name)?;

    // Get repo root and check .wf exists
    let repo_root = get_repo_root()?;
    let wf_dir = Path::new(&repo_root).join(WF_DIR);

    if !wf_dir.exists() {
        bail!("Not a wf project. Run 'wf init' first.");
    }

    // Check if task already exists
    let task_path = wf_dir.join(TASKS_DIR).join(format!("{}.md", name));
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

    println!("\nNext: wf start {}", name);

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

    content.push_str("# skip:          # 跳过不需要的 workflow 步骤\n");
    content.push_str("#   - cleanup\n");
    content.push_str("---\n\n");

    // Body
    if let Some(desc) = description {
        content.push_str(&format!("## Task: {}\n\n", name));
        content.push_str(desc);
        content.push('\n');
    } else {
        content.push_str(&format!(
            "<!-- 本文件同时作为 AI Worker 的 system prompt (cat task.md | claude -p) -->\n\
             <!-- 详细指南: .wf/lib/task-authoring-guide.md -->\n\n\
             ## Task: {}\n\n\
             ### 目标\n\n\
             <!-- 清晰描述要做什么 -->\n\n\
             ### 约束\n\n\
             <!-- 技术约束、代码规范、不该做什么 -->\n\n\
             ### 验收标准\n\n\
             - [ ] TODO\n",
            name
        ));
    }

    content
}
