use anyhow::{Context, Result};

use crate::util::shell::{run_command, run_command_success};

use super::Viewport;

pub struct TmuxViewport {
    session: String,
}

impl TmuxViewport {
    pub fn new(session: &str) -> Self {
        Self {
            session: session.to_string(),
        }
    }

    /// Check if a pane is running a process (has active command).
    /// This is TmuxViewport-specific (not part of the Viewport trait).
    pub fn pane_is_active(&self, name: &str) -> bool {
        let cmd = format!(
            "tmux list-panes -t '{}:{}' -F '#{{pane_current_command}}' 2>/dev/null",
            self.session, name
        );
        if let Ok(result) = run_command(&cmd) {
            if result.success {
                let cmd_name = result.stdout.trim();
                !matches!(cmd_name, "bash" | "zsh" | "sh" | "fish" | "")
            } else {
                false
            }
        } else {
            false
        }
    }

    fn session_exists(&self) -> bool {
        run_command_success(&format!("tmux has-session -t '{}' 2>/dev/null", self.session))
    }

    fn window_exists_raw(&self, window: &str) -> bool {
        run_command_success(&format!(
            "tmux list-windows -t '{}' -F '#{{window_name}}' 2>/dev/null | grep -qx '{}'",
            self.session, window
        ))
    }
}

impl Viewport for TmuxViewport {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn open(&self, name: &str, cwd: &str) -> Result<()> {
        if !self.session_exists() {
            let cmd = format!("tmux new-session -d -s '{}' -c '{}'", self.session, cwd);
            run_command(&cmd)
                .with_context(|| format!("Failed to create tmux session: {}", self.session))?;
        }

        if !self.window_exists_raw(name) {
            let cmd = format!(
                "tmux new-window -d -t '{}' -n '{}' -c '{}'",
                self.session, name, cwd
            );
            run_command(&cmd)
                .with_context(|| format!("Failed to create window: {}:{}", self.session, name))?;
        }

        Ok(())
    }

    fn send(&self, name: &str, text: &str) -> Result<()> {
        // Handle raw control characters (e.g., Ctrl+C for interrupt)
        if text == "\x03" {
            let cmd = format!("tmux send-keys -t '{}:{}' C-c", self.session, name);
            run_command(&cmd)
                .with_context(|| format!("Failed to send interrupt to {}:{}", self.session, name))?;
            return Ok(());
        }

        let escaped = text.replace('\'', "'\\''");
        let cmd = format!(
            "tmux send-keys -t '{}:{}' '{}' && tmux send-keys -t '{}:{}' Enter",
            self.session, name, escaped, self.session, name
        );
        run_command(&cmd)
            .with_context(|| format!("Failed to send keys to {}:{}", self.session, name))?;
        Ok(())
    }

    fn read(&self, name: &str, lines: usize) -> Result<Option<String>> {
        if !self.window_exists_raw(name) {
            return Ok(None);
        }

        let start = -(lines as i64);
        let cmd = format!(
            "tmux capture-pane -t '{}:{}' -p -J -S {}",
            self.session, name, start
        );
        let result = run_command(&cmd)?;
        if result.success {
            Ok(Some(result.stdout))
        } else {
            Ok(None)
        }
    }

    fn exists(&self, name: &str) -> bool {
        self.window_exists_raw(name)
    }

    fn close(&self, name: &str) -> Result<()> {
        if !self.window_exists_raw(name) {
            return Ok(());
        }
        let cmd = format!("tmux kill-window -t '{}:{}'", self.session, name);
        run_command(&cmd)
            .with_context(|| format!("Failed to kill window: {}:{}", self.session, name))?;
        Ok(())
    }

    fn attach(&self, name: &str) -> Result<()> {
        let cmd = format!("tmux select-window -t '{}:{}'", self.session, name);
        run_command(&cmd)
            .with_context(|| format!("Failed to select window: {}:{}", self.session, name))?;
        Ok(())
    }
}
