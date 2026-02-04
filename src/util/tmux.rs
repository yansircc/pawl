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
