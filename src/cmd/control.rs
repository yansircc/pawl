use anyhow::Result;

use crate::error::PawlError;
use crate::model::event::event_timestamp;
use crate::model::{Event, TaskStatus};

use super::common::Project;
use super::start::resume_workflow;

/// Stop the current task
pub fn stop(task_name: &str) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.replay_task(&task_name)?;
    let Some(state) = state else {
        return Err(PawlError::StateConflict {
            task: task_name.clone(),
            status: "pending".into(),
            message: format!("not started. Use 'pawl start {}' to begin", task_name),
        }.into());
    };

    match state.status {
        TaskStatus::Running | TaskStatus::Waiting => {}
        _ => {
            return Err(PawlError::StateConflict {
                task: task_name.clone(),
                status: state.status.to_string(),
                message: "not running".into(),
            }.into());
        }
    }

    // Send Ctrl+C to the viewport (if running)
    let session = project.session_name();

    if project.viewport.exists(&task_name) {
        eprintln!("Sending interrupt to {}:{}...", session, task_name);
        project.viewport.execute(&task_name, "\x03")?;
    }

    project.append_event(&task_name, &Event::TaskStopped {
        ts: event_timestamp(),
        step: state.current_step,
    })?;

    eprintln!("Task '{}' stopped.", task_name);

    // Output final state as JSON
    project.output_task_state(&task_name)?;

    Ok(())
}

/// Reset task — full reset or step-only reset
pub fn reset(task_name: &str, step_only: bool) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.replay_task(&task_name)?;

    if step_only {
        // Step-only reset: reset current step and continue execution
        let Some(state) = state else {
            return Err(PawlError::StateConflict {
                task: task_name.clone(),
                status: "pending".into(),
                message: "not started".into(),
            }.into());
        };

        let step_idx = state.current_step;
        if step_idx >= project.config.workflow.len() {
            return Err(PawlError::StateConflict {
                task: task_name.clone(),
                status: "completed".into(),
                message: format!("use 'pawl reset {}' for full reset", task_name),
            }.into());
        }

        match state.status {
            TaskStatus::Failed | TaskStatus::Stopped | TaskStatus::Waiting => {}
            TaskStatus::Running => {
                return Err(PawlError::StateConflict {
                    task: task_name.clone(),
                    status: "running".into(),
                    message: "already running".into(),
                }.into());
            }
            TaskStatus::Completed => {
                return Err(PawlError::StateConflict {
                    task: task_name.clone(),
                    status: "completed".into(),
                    message: format!("use 'pawl reset {}' for full reset", task_name),
                }.into());
            }
            TaskStatus::Pending => {
                return Err(PawlError::StateConflict {
                    task: task_name.clone(),
                    status: "pending".into(),
                    message: format!("not started. Use 'pawl start {}'", task_name),
                }.into());
            }
        }

        project.append_event(&task_name, &Event::StepReset {
            ts: event_timestamp(),
            step: step_idx,
            auto: false,
        })?;

        eprintln!("Reset step {}: {}", step_idx + 1, project.step_name(step_idx));
        resume_workflow(&project, &task_name)?;
    } else {
        // Full task reset
        let is_running = state
            .as_ref()
            .map(|s| s.status == TaskStatus::Running)
            .unwrap_or(false);

        if is_running {
            if project.viewport.exists(&task_name) {
                eprintln!("Stopping task viewport...");
                project.viewport.execute(&task_name, "\x03")?;
            }
        }

        project.append_event(&task_name, &Event::TaskReset { ts: event_timestamp() })?;

        eprintln!("Task '{}' reset to initial state.", task_name);

        // Only show git cleanup hints if resources actually exist
        let worktree_path = project.worktree_path(&task_name);
        let branch_name = format!("pawl/{}", task_name);
        let worktree_exists = worktree_path.exists();
        let branch_exists = crate::util::git::branch_exists(&branch_name);
        if worktree_exists || branch_exists {
            eprintln!("Note: Git resources are NOT automatically cleaned. Clean up manually:");
            if worktree_exists {
                eprintln!("  git worktree remove {} --force", worktree_path.display());
            }
            if branch_exists {
                eprintln!("  git branch -D {}", branch_name);
            }
        }
    }

    // Output final state as JSON
    project.output_task_state(&task_name)?;

    Ok(())
}
