use anyhow::{bail, Result};

use super::common::Project;

/// Show task logs (placeholder - logs not yet implemented)
pub fn run(task_name: &str) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    // Verify task exists
    let _task = project.load_task(&task_name)?;

    // Check if task has been started
    let Some(state) = project.status.get(&task_name) else {
        bail!("Task '{}' has not been started yet.", task_name);
    };

    println!("Task: {}", task_name);
    println!("Current step: {}", state.current_step + 1);
    println!("Status: {:?}", state.status);

    if let Some(msg) = &state.message {
        println!("Message: {}", msg);
    }

    println!();
    println!("Note: Detailed logging is not yet implemented.");
    println!("View the tmux window directly: wf enter {}", task_name);

    Ok(())
}
