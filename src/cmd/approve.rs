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
        bail!("Task '{}' not found.", task_name);
    };

    let step_idx = state.current_step;

    match state.status {
        TaskStatus::Running => {
            // Agent in tmux window reporting done — run verify first
            let step = &project.config.workflow[step_idx];

            match start::run_verify(&project, &task_name, step, step_idx)? {
                start::VerifyOutcome::Passed => {
                    println!("Verify passed.");
                }
                start::VerifyOutcome::Failed { feedback } => {
                    if !feedback.is_empty() {
                        eprintln!("Verify failed: {}", feedback);
                    }
                    bail!(
                        "Verification failed. Fix the issues and try 'wf done {}' again.",
                        task_name
                    );
                }
            }

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

            // Continue execution
            continue_execution(&project, &task_name)?;

            // Cleanup tmux window
            let _ = tmux::kill_window(&session, &task_name);
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
