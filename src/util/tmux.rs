use anyhow::{Context, Result};

use super::shell::{run_command, run_command_success};

/// Check if tmux is available
pub fn is_available() -> bool {
    run_command_success("command -v tmux")
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
    let cmd = format!("tmux send-keys -t '{}:{}' '{}' Enter", session, window, escaped);
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

/// Attach to a session
pub fn attach(session: &str) -> Result<()> {
    let cmd = format!("tmux attach-session -t '{}'", session);
    run_command(&cmd).with_context(|| format!("Failed to attach to session: {}", session))?;
    Ok(())
}

/// Kill a window
pub fn kill_window(session: &str, window: &str) -> Result<()> {
    let cmd = format!("tmux kill-window -t '{}:{}' 2>/dev/null || true", session, window);
    run_command(&cmd)?;
    Ok(())
}

/// Kill a session
pub fn kill_session(name: &str) -> Result<()> {
    let cmd = format!("tmux kill-session -t '{}' 2>/dev/null || true", name);
    run_command(&cmd)?;
    Ok(())
}

/// Capture pane content from a window
pub fn capture_pane(session: &str, window: &str, lines: usize) -> Result<String> {
    // Use negative start to capture from scrollback buffer
    let start = -(lines as i64);
    let cmd = format!(
        "tmux capture-pane -t '{}:{}' -p -S {} 2>/dev/null",
        session, window, start
    );
    let result = run_command(&cmd)?;
    if result.success {
        Ok(result.stdout)
    } else {
        Ok(String::new())
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
