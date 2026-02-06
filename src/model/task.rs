use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Task definition parsed from markdown file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDefinition {
    /// Task name (from frontmatter or filename)
    pub name: String,

    /// Dependencies (other task names)
    #[serde(default)]
    pub depends: Vec<String>,

    /// Steps to skip for this task (by step name)
    #[serde(default)]
    pub skip: Vec<String>,

    /// Task description (markdown body)
    #[serde(default)]
    pub description: String,
}

/// Frontmatter parsed from YAML
#[derive(Debug, Clone, Deserialize)]
struct Frontmatter {
    name: Option<String>,
    #[serde(default)]
    depends: Vec<String>,
    #[serde(default)]
    skip: Vec<String>,
}

impl TaskDefinition {
    /// Load a task from a markdown file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read task file: {}", path.display()))?;

        // Get name from filename if not in frontmatter
        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Self::parse(&content, &filename)
    }

    /// Parse task from markdown content
    pub fn parse(content: &str, default_name: &str) -> Result<Self> {
        let (frontmatter, body) = parse_frontmatter(content)?;

        let name = frontmatter
            .as_ref()
            .and_then(|f| f.name.clone())
            .unwrap_or_else(|| default_name.to_string());

        let depends = frontmatter
            .as_ref()
            .map(|f| f.depends.clone())
            .unwrap_or_default();

        let skip = frontmatter
            .as_ref()
            .map(|f| f.skip.clone())
            .unwrap_or_default();

        Ok(Self {
            name,
            depends,
            skip,
            description: body,
        })
    }

    /// Load all tasks from a directory
    pub fn load_all<P: AsRef<Path>>(tasks_dir: P) -> Result<Vec<Self>> {
        let tasks_dir = tasks_dir.as_ref();
        if !tasks_dir.exists() {
            return Ok(Vec::new());
        }

        let mut tasks = Vec::new();
        for entry in fs::read_dir(tasks_dir).context("Failed to read tasks directory")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                tasks.push(Self::load(&path)?);
            }
        }

        // Sort by name for consistent ordering
        tasks.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tasks)
    }
}

/// Parse YAML frontmatter from markdown content
fn parse_frontmatter(content: &str) -> Result<(Option<Frontmatter>, String)> {
    let content = content.trim_start();

    // Check for frontmatter delimiter
    if !content.starts_with("---") {
        return Ok((None, content.to_string()));
    }

    // Find the closing delimiter
    let after_first = &content[3..];
    let Some(end_pos) = after_first.find("\n---") else {
        bail!("Unclosed frontmatter: missing closing ---");
    };

    let yaml_content = &after_first[..end_pos].trim();
    let body_start = 3 + end_pos + 4; // skip "---\n" + yaml + "\n---"
    let body = content[body_start..].trim_start().to_string();

    // Parse YAML manually (simple key: value parsing)
    let frontmatter = parse_simple_yaml(yaml_content)?;

    Ok((Some(frontmatter), body))
}

/// Simple YAML parser for frontmatter (handles name, depends, and skip)
fn parse_simple_yaml(yaml: &str) -> Result<Frontmatter> {
    let mut name: Option<String> = None;
    let mut depends: Vec<String> = Vec::new();
    let mut skip: Vec<String> = Vec::new();
    let mut in_depends = false;
    let mut in_skip = false;

    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Check for list item
        if line.starts_with("  - ") || line.starts_with("\t- ") || line.starts_with("- ") {
            let item = line.trim().trim_start_matches('-').trim();
            if !item.is_empty() {
                if in_depends {
                    depends.push(item.to_string());
                } else if in_skip {
                    skip.push(item.to_string());
                }
            }
            continue;
        }

        // Check for key: value
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let value = value.trim();

            in_depends = false;
            in_skip = false;

            match key {
                "name" => {
                    name = Some(value.to_string());
                }
                "depends" => {
                    in_depends = true;
                    // Handle inline array: depends: [a, b, c]
                    if value.starts_with('[') && value.ends_with(']') {
                        let inner = &value[1..value.len() - 1];
                        for item in inner.split(',') {
                            let item = item.trim();
                            if !item.is_empty() {
                                depends.push(item.to_string());
                            }
                        }
                        in_depends = false;
                    }
                }
                "skip" => {
                    in_skip = true;
                    // Handle inline array: skip: [setup, cleanup]
                    if value.starts_with('[') && value.ends_with(']') {
                        let inner = &value[1..value.len() - 1];
                        for item in inner.split(',') {
                            let item = item.trim();
                            if !item.is_empty() {
                                skip.push(item.to_string());
                            }
                        }
                        in_skip = false;
                    }
                }
                _ => {}
            }
        }
    }

    Ok(Frontmatter { name, depends, skip })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: auth
depends:
  - database
  - config
---

## Task description

This is the task body.
"#;

        let task = TaskDefinition::parse(content, "default").unwrap();
        assert_eq!(task.name, "auth");
        assert_eq!(task.depends, vec!["database", "config"]);
        assert!(task.skip.is_empty());
        assert!(task.description.contains("Task description"));
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "# Just a markdown file\n\nNo frontmatter here.";
        let task = TaskDefinition::parse(content, "myfile").unwrap();
        assert_eq!(task.name, "myfile");
        assert!(task.depends.is_empty());
        assert!(task.skip.is_empty());
    }

    #[test]
    fn test_parse_inline_depends() {
        let content = r#"---
name: test
depends: [a, b, c]
---
Body
"#;
        let task = TaskDefinition::parse(content, "default").unwrap();
        assert_eq!(task.depends, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_skip() {
        let content = r#"---
name: hotfix
skip:
  - setup
  - cleanup
---
Quick fix task
"#;
        let task = TaskDefinition::parse(content, "default").unwrap();
        assert_eq!(task.name, "hotfix");
        assert_eq!(task.skip, vec!["setup", "cleanup"]);
    }

    #[test]
    fn test_parse_skip_inline() {
        let content = r#"---
name: hotfix
skip: [setup, cleanup]
---
Body
"#;
        let task = TaskDefinition::parse(content, "default").unwrap();
        assert_eq!(task.skip, vec!["setup", "cleanup"]);
    }
}
