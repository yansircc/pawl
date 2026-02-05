use anyhow::{Context, Result};

use super::shell::{run_command, run_command_success};

/// Result of capturing pane content
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureResult {
    /// Window exists and content was captured (may be empty)
    Content(String),
    /// Window no longer exists
    WindowGone,
}

/// Check if a session exists
pub fn session_exists(session: &str) -> bool {
    run_command_success(&format!("tmux has-session -t '{}' 2>/dev/null", session))
}

/// Check if a window exists in a session
pub fn window_exists(session: &str, window: &str) -> bool {
    run_command_success(&format!(
        "tmux list-windows -t '{}' -F '#{{window_name}}' 2>/dev/null | grep -qx '{}'",
        session, window
    ))
}

/// Create a new session (detached)
pub fn create_session(name: &str, dir: Option<&str>) -> Result<()> {
    let mut cmd = format!("tmux new-session -d -s '{}'", name);
    if let Some(dir) = dir {
        cmd.push_str(&format!(" -c '{}'", dir));
    }
    run_command(&cmd).with_context(|| format!("Failed to create tmux session: {}", name))?;
    Ok(())
}

/// Create a new window in a session
pub fn create_window(session: &str, window: &str, dir: Option<&str>) -> Result<()> {
    let mut cmd = format!("tmux new-window -t '{}' -n '{}'", session, window);
    if let Some(dir) = dir {
        cmd.push_str(&format!(" -c '{}'", dir));
    }
    run_command(&cmd).with_context(|| format!("Failed to create window: {}:{}", session, window))?;
    Ok(())
}

/// Send keys to a window
pub fn send_keys(session: &str, window: &str, keys: &str) -> Result<()> {
    // Escape single quotes in the keys
    let escaped = keys.replace('\'', "'\\''");
    // Send keys and Enter separately to ensure Enter is processed
    let cmd = format!(
        "tmux send-keys -t '{}:{}' '{}' && tmux send-keys -t '{}:{}' Enter",
        session, window, escaped, session, window
    );
    run_command(&cmd).with_context(|| format!("Failed to send keys to {}:{}", session, window))?;
    Ok(())
}

/// Send Ctrl+C to a window
pub fn send_interrupt(session: &str, window: &str) -> Result<()> {
    let cmd = format!("tmux send-keys -t '{}:{}' C-c", session, window);
    run_command(&cmd).with_context(|| format!("Failed to send interrupt to {}:{}", session, window))?;
    Ok(())
}

/// Switch to a window (bring to front)
pub fn select_window(session: &str, window: &str) -> Result<()> {
    let cmd = format!("tmux select-window -t '{}:{}'", session, window);
    run_command(&cmd).with_context(|| format!("Failed to select window: {}:{}", session, window))?;
    Ok(())
}

/// Kill a window (no-op if window doesn't exist)
pub fn kill_window(session: &str, window: &str) -> Result<()> {
    // Check if window exists first to avoid spurious errors
    if !window_exists(session, window) {
        return Ok(());
    }
    let cmd = format!("tmux kill-window -t '{}:{}'", session, window);
    run_command(&cmd).with_context(|| format!("Failed to kill window: {}:{}", session, window))?;
    Ok(())
}

/// Capture pane content from a window
/// Returns CaptureResult::WindowGone if the window no longer exists
/// Returns CaptureResult::Content with the content (may be empty) if window exists
pub fn capture_pane(session: &str, window: &str, lines: usize) -> Result<CaptureResult> {
    // First check if window exists
    if !window_exists(session, window) {
        return Ok(CaptureResult::WindowGone);
    }

    // Use negative start to capture from scrollback buffer
    // -J joins wrapped lines (prevents hard breaks at pane width)
    // -e includes escape sequences (we'll strip them later for clean output)
    let start = -(lines as i64);
    let cmd = format!(
        "tmux capture-pane -t '{}:{}' -p -J -S {}",
        session, window, start
    );
    let result = run_command(&cmd)?;
    if result.success {
        Ok(CaptureResult::Content(result.stdout))
    } else {
        // Command failed but window existed - treat as gone (race condition)
        Ok(CaptureResult::WindowGone)
    }
}

/// Check if a pane is running a process (has active command)
pub fn pane_is_active(session: &str, window: &str) -> bool {
    // Check if there's a running command in the pane
    let cmd = format!(
        "tmux list-panes -t '{}:{}' -F '#{{pane_current_command}}' 2>/dev/null",
        session, window
    );
    if let Ok(result) = run_command(&cmd) {
        if result.success {
            let cmd_name = result.stdout.trim();
            // If it's just a shell (bash, zsh, sh), no command is running
            !matches!(cmd_name, "bash" | "zsh" | "sh" | "fish" | "")
        } else {
            false
        }
    } else {
        false
    }
}

/// Extract Claude session ID from a tmux pane by examining environment variables
/// The session ID is stored in CLAUDE_SESSION_ID environment variable
pub fn extract_session_id(session: &str, window: &str) -> Option<String> {
    // First check if window exists
    if !window_exists(session, window) {
        return None;
    }

    // Get the pane's PID
    let cmd = format!(
        "tmux list-panes -t '{}:{}' -F '#{{pane_pid}}' 2>/dev/null",
        session, window
    );
    let result = run_command(&cmd).ok()?;
    if !result.success {
        return None;
    }

    let pane_pid = result.stdout.trim();
    if pane_pid.is_empty() {
        return None;
    }

    // Try to get CLAUDE_SESSION_ID from the process tree
    // Look in /proc on Linux or use lsof on macOS
    #[cfg(target_os = "linux")]
    {
        let env_cmd = format!("cat /proc/{}/environ 2>/dev/null | tr '\\0' '\\n' | grep '^CLAUDE_SESSION_ID=' | cut -d= -f2", pane_pid);
        if let Ok(result) = run_command(&env_cmd) {
            if result.success && !result.stdout.trim().is_empty() {
                return Some(result.stdout.trim().to_string());
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // On macOS, we can try to find the Claude process and its session ID
        // by looking at the process tree
        let ps_cmd = format!(
            "pgrep -P {} claude 2>/dev/null | head -1",
            pane_pid
        );
        if let Ok(result) = run_command(&ps_cmd) {
            if result.success && !result.stdout.trim().is_empty() {
                let claude_pid = result.stdout.trim();
                // Try to get environment from Claude process
                let env_cmd = format!(
                    "ps eww -p {} 2>/dev/null | grep -o 'CLAUDE_SESSION_ID=[^ ]*' | cut -d= -f2",
                    claude_pid
                );
                if let Ok(result) = run_command(&env_cmd) {
                    if result.success && !result.stdout.trim().is_empty() {
                        return Some(result.stdout.trim().to_string());
                    }
                }
            }
        }
    }

    None
}

/// Get transcript path for a Claude session ID
/// Claude stores transcripts in ~/.claude/projects/{hash}/{session_id}.jsonl
pub fn get_transcript_path(session_id: &str) -> Option<String> {
    // Get home directory
    let home = std::env::var("HOME").ok()?;
    let claude_dir = format!("{}/.claude/projects", home);

    // Find the transcript file by searching for session_id.jsonl
    let find_cmd = format!(
        "find '{}' -name '{}.jsonl' -type f 2>/dev/null | head -1",
        claude_dir, session_id
    );
    let result = run_command(&find_cmd).ok()?;

    if result.success && !result.stdout.trim().is_empty() {
        Some(result.stdout.trim().to_string())
    } else {
        None
    }
}

