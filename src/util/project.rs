use anyhow::Result;

use crate::error::PawlError;

/// Get the project root by walking up from cwd looking for `.pawl/` directory
pub fn get_project_root() -> Result<String> {
    let mut dir = std::env::current_dir()
        .map_err(|e| anyhow::anyhow!("Failed to get current directory: {}", e))?;

    loop {
        if dir.join(".pawl").is_dir() {
            return Ok(dir.to_string_lossy().to_string());
        }
        if !dir.pop() {
            return Err(PawlError::NotFound {
                message: "Not a pawl project (no .pawl/ found). Run 'pawl init' first.".into(),
            }
            .into());
        }
    }
}

/// Validate that a name is a valid task name (filesystem-safe)
pub fn validate_task_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(PawlError::Validation {
            message: "Task name cannot be empty".into(),
        }
        .into());
    }

    let invalid_chars = [' ', '~', '^', ':', '?', '*', '[', '@', '{', '\\', '/'];
    for c in invalid_chars {
        if name.contains(c) {
            return Err(PawlError::Validation {
                message: format!("Task name cannot contain '{}'", c),
            }
            .into());
        }
    }

    if name.starts_with('-') {
        return Err(PawlError::Validation {
            message: "Task name cannot start with '-'".into(),
        }
        .into());
    }
    if name.starts_with('.') {
        return Err(PawlError::Validation {
            message: "Task name cannot start with '.'".into(),
        }
        .into());
    }
    if name.ends_with(".lock") {
        return Err(PawlError::Validation {
            message: "Task name cannot end with '.lock'".into(),
        }
        .into());
    }
    if name.contains("..") {
        return Err(PawlError::Validation {
            message: "Task name cannot contain '..'".into(),
        }
        .into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_task_name() {
        // Valid names
        assert!(validate_task_name("auth").is_ok());
        assert!(validate_task_name("user-auth").is_ok());
        assert!(validate_task_name("feature_123").is_ok());

        // Invalid names
        assert!(validate_task_name("").is_err());
        assert!(validate_task_name("-invalid").is_err());
        assert!(validate_task_name(".invalid").is_err());
        assert!(validate_task_name("name.lock").is_err());
        assert!(validate_task_name("with space").is_err());
        assert!(validate_task_name("with..dots").is_err());
        assert!(validate_task_name("with/slash").is_err());
    }
}
