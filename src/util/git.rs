use anyhow::{Context, Result};

use crate::error::PawlError;
use super::shell::{run_command_output, run_command_success};

/// Get the root directory of the git repository
pub fn get_repo_root() -> Result<String> {
    run_command_output("git rev-parse --show-toplevel")
        .context("Failed to get git repository root. Are you in a git repository?")
}

/// Validate that a name is a valid git branch suffix
pub fn validate_branch_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(PawlError::Validation { message: "Task name cannot be empty".into() }.into());
    }

    // Check for invalid characters
    let invalid_chars = [' ', '~', '^', ':', '?', '*', '[', '@', '{', '\\'];
    for c in invalid_chars {
        if name.contains(c) {
            return Err(PawlError::Validation {
                message: format!("Task name cannot contain '{}'", c),
            }.into());
        }
    }

    // Check for invalid patterns
    if name.starts_with('-') {
        return Err(PawlError::Validation { message: "Task name cannot start with '-'".into() }.into());
    }
    if name.starts_with('.') {
        return Err(PawlError::Validation { message: "Task name cannot start with '.'".into() }.into());
    }
    if name.ends_with(".lock") {
        return Err(PawlError::Validation { message: "Task name cannot end with '.lock'".into() }.into());
    }
    if name.contains("..") {
        return Err(PawlError::Validation { message: "Task name cannot contain '..'".into() }.into());
    }

    // Validate with git check-ref-format
    let test_ref = format!("refs/heads/pawl/{}", name);
    if !run_command_success(&format!("git check-ref-format '{}'", test_ref)) {
        return Err(PawlError::Validation {
            message: format!("'{}' is not a valid git branch name", name),
        }.into());
    }

    Ok(())
}

/// Check if a git branch exists
pub fn branch_exists(branch_name: &str) -> bool {
    run_command_success(&format!("git rev-parse --verify refs/heads/{}", branch_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_branch_name() {
        // Valid names
        assert!(validate_branch_name("auth").is_ok());
        assert!(validate_branch_name("user-auth").is_ok());
        assert!(validate_branch_name("feature_123").is_ok());

        // Invalid names
        assert!(validate_branch_name("").is_err());
        assert!(validate_branch_name("-invalid").is_err());
        assert!(validate_branch_name(".invalid").is_err());
        assert!(validate_branch_name("name.lock").is_err());
        assert!(validate_branch_name("with space").is_err());
        assert!(validate_branch_name("with..dots").is_err());
    }
}
