use anyhow::Result;

use crate::error::PawlError;
use super::common::Project;

/// Enter the task's viewport
pub fn run(task_name: &str) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    // Verify task exists
    let _task = project.load_task(&task_name)?;

    let session = project.session_name();

    // Check if viewport exists
    if !project.viewport.exists(&task_name) {
        return Err(PawlError::NotFound {
            message: format!("Viewport '{}:{}' does not exist. Task may not have been started.", session, task_name),
        }.into());
    }

    // Attach to the viewport
    project.viewport.attach(&task_name)?;

    eprintln!("Switched to {}:{}", session, task_name);

    Ok(())
}
