use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::process::{Command, Output, Stdio};

/// Result of a command execution
#[derive(Debug)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

impl CommandResult {
    fn from_output(output: Output) -> Self {
        Self {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            success: output.status.success(),
        }
    }
}

/// Run a shell command and return the result
pub fn run_command(cmd: &str) -> Result<CommandResult> {
    run_command_with_options(cmd, None, None)
}

/// Run a shell command with environment variables
pub fn run_command_with_env(cmd: &str, env: &HashMap<String, String>) -> Result<CommandResult> {
    run_command_with_options(cmd, None, Some(env))
}

/// Run a shell command with all options
pub fn run_command_with_options(
    cmd: &str,
    dir: Option<&str>,
    env: Option<&HashMap<String, String>>,
) -> Result<CommandResult> {
    let mut command = Command::new("sh");
    command.arg("-c").arg(cmd);

    if let Some(dir) = dir {
        command.current_dir(dir);
    }

    if let Some(env) = env {
        for (key, value) in env {
            command.env(key, value);
        }
    }

    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("Failed to execute command: {}", cmd))?;

    Ok(CommandResult::from_output(output))
}

/// Run a command and return stdout, failing if command fails
pub fn run_command_output(cmd: &str) -> Result<String> {
    let result = run_command(cmd)?;
    if !result.success {
        bail!(
            "Command failed with exit code {}: {}\nstderr: {}",
            result.exit_code,
            cmd,
            result.stderr
        );
    }
    Ok(result.stdout.trim().to_string())
}

/// Run a command and check if it succeeded (for boolean checks)
pub fn run_command_success(cmd: &str) -> bool {
    run_command(cmd).map(|r| r.success).unwrap_or(false)
}

/// Spawn a command in the background (fire-and-forget)
pub fn spawn_background(cmd: &str) -> Result<()> {
    Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to spawn background command: {}", cmd))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_command() {
        let result = run_command("echo hello").unwrap();
        assert!(result.success);
        assert_eq!(result.stdout.trim(), "hello");
    }

    #[test]
    fn test_run_command_failure() {
        let result = run_command("exit 1").unwrap();
        assert!(!result.success);
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_run_command_output() {
        let output = run_command_output("echo hello").unwrap();
        assert_eq!(output, "hello");
    }
}
