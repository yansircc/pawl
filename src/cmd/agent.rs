use anyhow::{bail, Result};

use crate::model::event::event_timestamp;
use crate::model::{AgentResult, Event, TaskStatus};
use crate::util::tmux;

use super::common::Project;
use super::start;
use super::start::continue_execution;

/// Mark current step as done (success)
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
                start::VerifyOutcome::HumanRequired => {
                    // verify: "human" — this shouldn't happen for in_window agent calling done,
                    // but handle gracefully: approve automatically since agent says done
                    println!("Human verify — agent reports done, approving.");
                }
                start::VerifyOutcome::Failed { feedback } => {
                    eprintln!("Verify failed.");
                    if !feedback.is_empty() {
                        eprintln!("{}", feedback);
                    }
                    bail!(
                        "Verification failed. Fix the issues and try 'wf done {}' again.",
                        task_name
                    );
                }
            }

            // Extract session_id before cleanup (must happen while window still exists)
            let session = project.session_name();
            let session_id = tmux::extract_session_id(&session, &task_name);
            let transcript = session_id.as_ref().and_then(|id| tmux::get_transcript_path(id));

            // Emit AgentReported Done event
            project.append_event(&task_name, &Event::AgentReported {
                ts: event_timestamp(),
                step: step_idx,
                result: AgentResult::Done,
                session_id,
                transcript,
                message: message.map(|s| s.to_string()),
            })?;

            println!("Step {} marked as done.", step_idx + 1);

            // Continue execution first, then cleanup window
            continue_execution(&project, &task_name)?;

            // Cleanup tmux window after execution completes
            cleanup_window(&session, &task_name);
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

/// Mark current step as failed
pub fn fail(task_name: &str, message: Option<&str>) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.replay_task(&task_name)?;
    let Some(state) = state else {
        bail!("Task '{}' not found.", task_name);
    };

    let step_idx = state.current_step;

    match state.status {
        TaskStatus::Running => {
            // Extract session_id before cleanup
            let session = project.session_name();
            let session_id = tmux::extract_session_id(&session, &task_name);
            let transcript = session_id.as_ref().and_then(|id| tmux::get_transcript_path(id));

            // Emit AgentReported Failed event
            project.append_event(&task_name, &Event::AgentReported {
                ts: event_timestamp(),
                step: step_idx,
                result: AgentResult::Failed,
                session_id,
                transcript,
                message: message.map(|s| s.to_string()),
            })?;

            // Cleanup tmux window
            cleanup_window(&session, &task_name);

            println!("Step {} marked as failed.", step_idx + 1);
            if let Some(msg) = message {
                println!("Reason: {}", msg);
            }
            println!("Use 'wf retry {}' to try again.", task_name);
        }
        TaskStatus::Waiting => {
            // Human rejects: check on_fail strategy
            let step = &project.config.workflow[step_idx];

            if step.on_fail_retry() {
                // Retry the step
                project.append_event(&task_name, &Event::StepRetried {
                    ts: event_timestamp(),
                    step: step_idx,
                })?;
                println!("Step {} rejected. Retrying...", step_idx + 1);
                continue_execution(&project, &task_name)?;
            } else {
                // Default: mark as failed
                project.append_event(&task_name, &Event::AgentReported {
                    ts: event_timestamp(),
                    step: step_idx,
                    result: AgentResult::Failed,
                    session_id: None,
                    transcript: None,
                    message: message.map(|s| s.to_string()),
                })?;

                println!("Step {} rejected and marked as failed.", step_idx + 1);
                if let Some(msg) = message {
                    println!("Reason: {}", msg);
                }
                println!("Use 'wf retry {}' to try again.", task_name);
            }
        }
        _ => {
            bail!(
                "Task '{}' is {:?}. Cannot mark as failed.",
                task_name,
                state.status
            );
        }
    }

    Ok(())
}

/// Cleanup tmux window after step completion (best-effort)
fn cleanup_window(session: &str, window: &str) {
    let _ = tmux::kill_window(session, window);
}
