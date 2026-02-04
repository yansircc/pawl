use anyhow::{bail, Result};

use crate::util::tmux;

use super::common::Project;

/// Enter the task's tmux window
pub fn run(task_name: &str) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    // Verify task exists
    let _task = project.load_task(&task_name)?;

    let session = project.session_name();
    let window = &task_name;

    // Check if window exists
    if !tmux::window_exists(&session, window) {
        bail!(
            "Window '{}:{}' does not exist. Task may not have been started.",
            session,
            window
        );
    }

    // Select the window
    tmux::select_window(&session, window)?;

    println!("Switched to {}:{}", session, window);

    Ok(())
}
