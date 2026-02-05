use anyhow::{bail, Context, Result};

use super::shell::{run_command_output, run_command_success};

/// Get the root directory of the git repository
pub fn get_repo_root() -> Result<String> {
    run_command_output("git rev-parse --show-toplevel")
        .context("Failed to get git repository root. Are you in a git repository?")
}

/// Validate that a name is a valid git branch suffix
pub fn validate_branch_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Task name cannot be empty");
    }

    // Check for invalid characters
    let invalid_chars = [' ', '~', '^', ':', '?', '*', '[', '@', '{', '\\'];
    for c in invalid_chars {
        if name.contains(c) {
            bail!("Task name cannot contain '{}'", c);
        }
    }

    // Check for invalid patterns
    if name.starts_with('-') {
        bail!("Task name cannot start with '-'");
    }
    if name.starts_with('.') {
        bail!("Task name cannot start with '.'");
    }
    if name.ends_with(".lock") {
        bail!("Task name cannot end with '.lock'");
    }
    if name.contains("..") {
        bail!("Task name cannot contain '..'");
    }

    // Validate with git check-ref-format
    let test_ref = format!("refs/heads/wf/{}", name);
    if !run_command_success(&format!("git check-ref-format '{}'", test_ref)) {
        bail!("'{}' is not a valid git branch name", name);
    }

    Ok(())
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
