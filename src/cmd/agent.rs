use anyhow::{bail, Result};

use crate::model::{StepStatus, TaskStatus};

use super::common::Project;
use super::start::continue_execution;

/// Mark current step as done (success)
pub fn done(task_name: &str, message: Option<&str>) -> Result<()> {
    let mut project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.status.get_mut(&task_name);
    let Some(state) = state else {
        bail!("Task '{}' not found.", task_name);
    };

    if state.status != TaskStatus::Running {
        bail!(
            "Task '{}' is not running (status: {:?}). Cannot mark as done.",
            task_name,
            state.status
        );
    }

    let step_idx = state.current_step;

    // Mark step as success
    state.mark_step(step_idx, StepStatus::Success);
    state.current_step += 1;
    state.message = message.map(|s| s.to_string());
    project.save_status()?;

    println!("Step {} marked as done.", step_idx + 1);

    // Continue execution
    continue_execution(&mut project, &task_name)?;

    Ok(())
}

/// Mark current step as failed
pub fn fail(task_name: &str, message: Option<&str>) -> Result<()> {
    let mut project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.status.get_mut(&task_name);
    let Some(state) = state else {
        bail!("Task '{}' not found.", task_name);
    };

    if state.status != TaskStatus::Running {
        bail!(
            "Task '{}' is not running (status: {:?}). Cannot mark as failed.",
            task_name,
            state.status
        );
    }

    let step_idx = state.current_step;

    // Mark step as failed
    state.mark_step(step_idx, StepStatus::Failed);
    state.status = TaskStatus::Failed;
    state.message = message.map(|s| s.to_string());
    state.touch();
    project.save_status()?;

    println!("Step {} marked as failed.", step_idx + 1);
    if let Some(msg) = message {
        println!("Reason: {}", msg);
    }
    println!("Use 'wf retry {}' to try again.", task_name);

    Ok(())
}

/// Mark current step as blocked (needs human intervention)
pub fn block(task_name: &str, message: Option<&str>) -> Result<()> {
    let mut project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.status.get_mut(&task_name);
    let Some(state) = state else {
        bail!("Task '{}' not found.", task_name);
    };

    if state.status != TaskStatus::Running {
        bail!(
            "Task '{}' is not running (status: {:?}). Cannot mark as blocked.",
            task_name,
            state.status
        );
    }

    let step_idx = state.current_step;

    // Mark step as blocked
    state.mark_step(step_idx, StepStatus::Blocked);
    state.status = TaskStatus::Waiting;
    state.message = message.map(|s| s.to_string());
    state.touch();
    project.save_status()?;

    println!("Step {} marked as blocked.", step_idx + 1);
    if let Some(msg) = message {
        println!("Reason: {}", msg);
    }
    println!("Resolve the issue and use 'wf next {}' to continue.", task_name);

    Ok(())
}
