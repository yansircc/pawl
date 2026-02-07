use anyhow::{bail, Result};

use crate::model::event::event_timestamp;
use crate::model::{Event, TaskStatus};
use crate::util::tmux;

use super::common::Project;
use super::start;
use super::start::continue_execution;

/// Mark current step as done (approve waiting step, or complete in_window step)
pub fn done(task_name: &str, message: Option<&str>) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.replay_task(&task_name)?;
    let Some(state) = state else {
        bail!("Task '{}' has not been started. Use 'wf start {}' to begin.", task_name, task_name);
    };

    let step_idx = state.current_step;

    match state.status {
        TaskStatus::Running => {
            // Agent in tmux window reporting done — go through unified pipeline
            let step = &project.config.workflow[step_idx];
            let session = project.session_name();

            // Emit StepCompleted with exit_code 0 (agent says done)
            project.append_event(&task_name, &Event::StepCompleted {
                ts: event_timestamp(),
                step: step_idx,
                exit_code: 0,
                duration: None,
                stdout: message.map(|s| s.to_string()),
                stderr: None,
            })?;

            println!("Step {} marked as done.", step_idx + 1);

            // Unified pipeline: verify + apply_on_fail
            let should_continue = start::handle_step_completion(
                &project, &task_name, step_idx, 0, step
            )?;

            if should_continue {
                continue_execution(&project, &task_name)?;
            }

            // Cleanup tmux window — but not if retrying (apply_on_fail re-sent command)
            let new_state = project.replay_task(&task_name)?;
            let retrying = matches!(&new_state,
                Some(s) if s.status == TaskStatus::Running && s.current_step == step_idx
            );
            if !retrying {
                let _ = tmux::kill_window(&session, &task_name);
            }
        }
        TaskStatus::Waiting => {
            // Human approval: emit StepApproved and continue
            project.append_event(&task_name, &Event::StepApproved {
                ts: event_timestamp(),
                step: step_idx,
            })?;

            println!("Step {} approved.", step_idx + 1);
            continue_execution(&project, &task_name)?;
        }
        _ => {
            bail!(
                "Task '{}' is {:?}. Cannot mark as done.",
                task_name,
                state.status
            );
        }
    }

    Ok(())
}
