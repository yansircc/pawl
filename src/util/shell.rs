use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

/// Result of a command execution
#[derive(Debug)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

static STDERR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Run a shell command with env, streaming stdout line-by-line through a callback.
/// stderr is redirected to a temp file internally to avoid pipe deadlocks.
/// For batch (non-streaming) usage, pass `|_| {}` as the callback.
pub fn run_command(
    cmd: &str,
    env: &HashMap<String, String>,
    mut on_line: impl FnMut(&str),
) -> Result<CommandResult> {
    let id = STDERR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let stderr_path = std::env::temp_dir().join(format!("pawl-{}-{}.stderr", std::process::id(), id));

    let stderr_out = std::fs::File::create(&stderr_path)
        .with_context(|| "Failed to create stderr temp file")?;

    let mut command = Command::new("sh");
    command.arg("-c").arg(cmd);
    for (key, value) in env {
        command.env(key, value);
    }

    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr_out))
        .spawn()
        .with_context(|| format!("Failed to spawn command: {}", cmd))?;

    let stdout_pipe = child.stdout.take().expect("stdout was piped");
    let reader = BufReader::new(stdout_pipe);
    let mut stdout_buf = String::new();

    for line in reader.lines() {
        let line = line.with_context(|| "Failed to read stdout line")?;
        on_line(&line);
        stdout_buf.push_str(&line);
        stdout_buf.push('\n');
    }

    let status = child.wait().with_context(|| "Failed to wait for child process")?;
    let stderr = std::fs::read_to_string(&stderr_path).unwrap_or_default();
    let _ = std::fs::remove_file(&stderr_path);

    Ok(CommandResult {
        stdout: stdout_buf,
        stderr,
        exit_code: status.code().unwrap_or(-1),
        success: status.success(),
    })
}

/// Run a command and check if it succeeded (for boolean checks)
pub fn run_command_success(cmd: &str) -> bool {
    run_command(cmd, &HashMap::new(), |_| {}).map(|r| r.success).unwrap_or(false)
}

/// Run a shell command with env, redirecting stdout to a file.
/// Unlike run_command (which uses a pipe), this won't hang if the child
/// forks background processes that inherit stdout — child.wait() returns
/// as soon as the direct child exits, regardless of grandchild fd inheritance.
/// The stdout file doubles as the live stream file for dashboard consumption.
pub fn run_command_to_file(
    cmd: &str,
    env: &HashMap<String, String>,
    stdout_path: &Path,
) -> Result<CommandResult> {
    let id = STDERR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let stderr_path = std::env::temp_dir().join(format!("pawl-{}-{}.stderr", std::process::id(), id));

    let stderr_out = std::fs::File::create(&stderr_path)
        .with_context(|| "Failed to create stderr temp file")?;

    let stdout_out = std::fs::File::create(stdout_path)
        .with_context(|| "Failed to create stdout file")?;

    let mut command = Command::new("sh");
    command.arg("-c").arg(cmd);
    for (key, value) in env {
        command.env(key, value);
    }

    let mut child = command
        .stdout(Stdio::from(stdout_out))
        .stderr(Stdio::from(stderr_out))
        .spawn()
        .with_context(|| format!("Failed to spawn command: {}", cmd))?;

    let status = child.wait().with_context(|| "Failed to wait for child process")?;

    let stdout = std::fs::read_to_string(stdout_path).unwrap_or_default();
    let stderr = std::fs::read_to_string(&stderr_path).unwrap_or_default();
    let _ = std::fs::remove_file(&stderr_path);

    Ok(CommandResult {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
        success: status.success(),
    })
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
    fn test_run_command_batch() {
        let result = run_command("echo hello", &HashMap::new(), |_| {}).unwrap();
        assert!(result.success);
        assert_eq!(result.stdout.trim(), "hello");
    }

    #[test]
    fn test_run_command_failure() {
        let result = run_command("exit 1", &HashMap::new(), |_| {}).unwrap();
        assert!(!result.success);
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_run_command_streaming() {
        let env = HashMap::new();
        let mut lines = Vec::new();

        let result = run_command(
            "echo line1; echo line2; echo line3",
            &env,
            |line| lines.push(line.to_string()),
        )
        .unwrap();

        assert!(result.success);
        assert_eq!(lines, vec!["line1", "line2", "line3"]);
        assert_eq!(result.stdout.trim(), "line1\nline2\nline3");
    }

    #[test]
    fn test_run_command_stderr() {
        let result = run_command("echo out; echo err >&2; exit 42", &HashMap::new(), |_| {}).unwrap();

        assert!(!result.success);
        assert_eq!(result.exit_code, 42);
        assert_eq!(result.stdout.trim(), "out");
        assert_eq!(result.stderr.trim(), "err");
    }
}
