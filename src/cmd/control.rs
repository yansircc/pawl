use anyhow::{bail, Result};

use crate::model::{StepStatus, TaskStatus};
use crate::util::tmux;

use super::common::Project;
use super::start::continue_execution;

/// Advance to next step (pass checkpoint or continue after in_window)
pub fn next(task_name: &str) -> Result<()> {
    let mut project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let status = {
        let state = project.status.get(&task_name);
        let Some(state) = state else {
            bail!("Task '{}' has not been started. Use 'wf start {}'", task_name, task_name);
        };
        state.status
    };

    match status {
        TaskStatus::Waiting => {
            // Advance to next step
            {
                let state = project.status.get_mut(&task_name).unwrap();
                state.current_step += 1;
                state.status = TaskStatus::Running;
                state.touch();
            }
            project.save_status()?;

            println!("Continuing task: {}", task_name);
            continue_execution(&mut project, &task_name)?;
        }
        TaskStatus::Running => {
            bail!("Task '{}' is running. Wait for it to pause or complete.", task_name);
        }
        TaskStatus::Completed => {
            bail!("Task '{}' is already completed.", task_name);
        }
        TaskStatus::Failed | TaskStatus::Stopped => {
            bail!("Task '{}' is {}. Use 'wf retry {}' to continue.", task_name,
                  if status == TaskStatus::Failed { "failed" } else { "stopped" },
                  task_name);
        }
        TaskStatus::Pending => {
            bail!("Task '{}' has not been started. Use 'wf start {}'", task_name, task_name);
        }
    }

    Ok(())
}

/// Retry the current step
pub fn retry(task_name: &str) -> Result<()> {
    let mut project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let (status, current_step) = {
        let state = project.status.get(&task_name);
        let Some(state) = state else {
            bail!("Task '{}' has not been started. Use 'wf start {}'", task_name, task_name);
        };
        (state.status, state.current_step)
    };

    match status {
        TaskStatus::Failed | TaskStatus::Stopped | TaskStatus::Waiting => {
            // Keep current_step the same, just re-run
            {
                let state = project.status.get_mut(&task_name).unwrap();
                state.status = TaskStatus::Running;
                state.message = None;
                state.touch();
            }
            project.save_status()?;

            println!("Retrying task: {} at step {}", task_name, current_step + 1);
            continue_execution(&mut project, &task_name)?;
        }
        TaskStatus::Running => {
            bail!("Task '{}' is already running.", task_name);
        }
        TaskStatus::Completed => {
            bail!("Task '{}' is completed. Use 'wf reset {}' to restart.", task_name, task_name);
        }
        TaskStatus::Pending => {
            bail!("Task '{}' has not been started. Use 'wf start {}'", task_name, task_name);
        }
    }

    Ok(())
}

/// Go back to the previous step
pub fn back(task_name: &str) -> Result<()> {
    let mut project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let (current_step, step_name) = {
        let state = project.status.get(&task_name);
        let Some(state) = state else {
            bail!("Task '{}' has not been started.", task_name);
        };

        if state.current_step == 0 {
            bail!("Already at the first step.");
        }

        let new_step = state.current_step - 1;
        let step_name = project.config.workflow[new_step].name.clone();
        (new_step, step_name)
    };

    {
        let state = project.status.get_mut(&task_name).unwrap();
        state.current_step = current_step;
        state.status = TaskStatus::Waiting;
        state.message = None;
        state.touch();
    }
    project.save_status()?;

    println!(
        "Moved back to step {}: {}",
        current_step + 1,
        step_name
    );
    println!("Use 'wf retry {}' to re-run this step.", task_name);

    Ok(())
}

/// Skip the current step
pub fn skip(task_name: &str) -> Result<()> {
    let mut project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let (step_idx, step_name) = {
        let state = project.status.get(&task_name);
        let Some(state) = state else {
            bail!("Task '{}' has not been started.", task_name);
        };

        let step_idx = state.current_step;
        if step_idx >= project.config.workflow.len() {
            bail!("No more steps to skip.");
        }

        let step_name = project.config.workflow[step_idx].name.clone();
        (step_idx, step_name)
    };

    // Mark current step as skipped
    {
        let state = project.status.get_mut(&task_name).unwrap();
        state.mark_step(step_idx, StepStatus::Skipped);
        state.current_step += 1;
        state.status = TaskStatus::Running;
        state.touch();
    }
    project.save_status()?;

    println!("Skipped step {}: {}", step_idx + 1, step_name);
    continue_execution(&mut project, &task_name)?;

    Ok(())
}

/// Stop the current task
pub fn stop(task_name: &str) -> Result<()> {
    let mut project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let status = {
        let state = project.status.get(&task_name);
        let Some(state) = state else {
            bail!("Task '{}' has not been started.", task_name);
        };
        state.status
    };

    if status != TaskStatus::Running {
        bail!("Task '{}' is not running (status: {:?}).", task_name, status);
    }

    // Send Ctrl+C to the tmux window
    let session = project.session_name();
    let window = &task_name;

    if tmux::window_exists(&session, window) {
        println!("Sending interrupt to {}:{}...", session, window);
        tmux::send_interrupt(&session, window)?;
    }

    {
        let state = project.status.get_mut(&task_name).unwrap();
        state.status = TaskStatus::Stopped;
        state.touch();
    }
    project.save_status()?;

    println!("Task '{}' stopped.", task_name);
    println!("Use 'wf retry {}' to continue.", task_name);

    Ok(())
}

/// Reset task to initial state
pub fn reset(task_name: &str) -> Result<()> {
    let mut project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    // Stop if running
    let is_running = project
        .status
        .get(&task_name)
        .map(|s| s.status == TaskStatus::Running)
        .unwrap_or(false);

    if is_running {
        let session = project.session_name();
        if tmux::window_exists(&session, &task_name) {
            println!("Stopping task window...");
            tmux::send_interrupt(&session, &task_name)?;
        }
    }

    // Remove state
    project.status.remove(&task_name);
    project.save_status()?;

    println!("Task '{}' reset to initial state.", task_name);
    println!("Note: Git resources (branch, worktree) are NOT automatically cleaned.");
    println!("Clean up manually if needed:");
    println!("  git worktree remove .wf/worktrees/{} --force", task_name);
    println!("  git branch -D wf/{}", task_name);

    Ok(())
}

/// Internal: called when in_window command exits
pub fn on_exit(task_name: &str) -> Result<()> {
    let mut project = Project::load()?;

    let status = project.status.get(task_name).map(|s| s.status);

    // If task doesn't exist or already not running, nothing to do
    let Some(status) = status else {
        return Ok(());
    };

    if status != TaskStatus::Running {
        return Ok(());
    }

    // Fallback: mark as needing attention
    {
        let state = project.status.get_mut(task_name).unwrap();
        state.status = TaskStatus::Waiting;
        state.message = Some("Process exited without calling wf done/fail/block".to_string());
        state.touch();
    }
    project.save_status()?;

    eprintln!(
        "Warning: Task '{}' window exited without status. Use 'wf next {}' to continue.",
        task_name, task_name
    );

    Ok(())
}
