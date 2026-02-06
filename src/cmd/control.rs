use anyhow::{bail, Result};

use crate::model::event::event_timestamp;
use crate::model::{Event, TaskStatus};
use crate::util::tmux;

use super::common::Project;
use super::start::continue_execution;

/// Advance to next step (pass checkpoint or continue after in_window)
pub fn next(task_name: &str) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.replay_task(&task_name)?;
    let Some(state) = state else {
        bail!("Task '{}' has not been started. Use 'wf start {}'", task_name, task_name);
    };

    match state.status {
        TaskStatus::Waiting => {
            project.append_event(&task_name, &Event::CheckpointPassed {
                ts: event_timestamp(),
                step: state.current_step,
            })?;

            println!("Continuing task: {}", task_name);
            continue_execution(&project, &task_name)?;
        }
        TaskStatus::Running => {
            bail!("Task '{}' is running. Wait for it to pause or complete.", task_name);
        }
        TaskStatus::Completed => {
            bail!("Task '{}' is already completed.", task_name);
        }
        TaskStatus::Failed | TaskStatus::Stopped => {
            bail!("Task '{}' is {}. Use 'wf retry {}' to continue.", task_name,
                  if state.status == TaskStatus::Failed { "failed" } else { "stopped" },
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
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.replay_task(&task_name)?;
    let Some(state) = state else {
        bail!("Task '{}' has not been started. Use 'wf start {}'", task_name, task_name);
    };

    match state.status {
        TaskStatus::Failed | TaskStatus::Stopped | TaskStatus::Waiting => {
            project.append_event(&task_name, &Event::StepRetried {
                ts: event_timestamp(),
                step: state.current_step,
            })?;

            println!("Retrying task: {} at step {}", task_name, state.current_step + 1);
            continue_execution(&project, &task_name)?;
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
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.replay_task(&task_name)?;
    let Some(state) = state else {
        bail!("Task '{}' has not been started.", task_name);
    };

    if state.current_step == 0 {
        bail!("Already at the first step.");
    }

    let to_step = state.current_step - 1;
    let step_name = project.config.workflow[to_step].name.clone();

    project.append_event(&task_name, &Event::StepRolledBack {
        ts: event_timestamp(),
        from_step: state.current_step,
        to_step,
    })?;

    println!(
        "Moved back to step {}: {}",
        to_step + 1,
        step_name
    );
    println!("Use 'wf retry {}' to re-run this step.", task_name);

    Ok(())
}

/// Skip the current step
pub fn skip(task_name: &str) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.replay_task(&task_name)?;
    let Some(state) = state else {
        bail!("Task '{}' has not been started.", task_name);
    };

    let step_idx = state.current_step;
    if step_idx >= project.config.workflow.len() {
        bail!("No more steps to skip.");
    }

    let step_name = project.config.workflow[step_idx].name.clone();

    project.append_event(&task_name, &Event::StepSkipped {
        ts: event_timestamp(),
        step: step_idx,
    })?;

    println!("Skipped step {}: {}", step_idx + 1, step_name);
    continue_execution(&project, &task_name)?;

    Ok(())
}

/// Stop the current task
pub fn stop(task_name: &str) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.replay_task(&task_name)?;
    let Some(state) = state else {
        bail!("Task '{}' has not been started.", task_name);
    };

    if state.status != TaskStatus::Running {
        bail!("Task '{}' is not running (status: {:?}).", task_name, state.status);
    }

    // Send Ctrl+C to the tmux window
    let session = project.session_name();
    let window = &task_name;

    if tmux::window_exists(&session, window) {
        println!("Sending interrupt to {}:{}...", session, window);
        tmux::send_interrupt(&session, window)?;
    }

    project.append_event(&task_name, &Event::TaskStopped {
        ts: event_timestamp(),
        step: state.current_step,
    })?;

    println!("Task '{}' stopped.", task_name);
    println!("Use 'wf retry {}' to continue.", task_name);

    Ok(())
}

/// Reset task to initial state
pub fn reset(task_name: &str) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    // Stop if running
    let state = project.replay_task(&task_name)?;
    let is_running = state
        .as_ref()
        .map(|s| s.status == TaskStatus::Running)
        .unwrap_or(false);

    if is_running {
        let session = project.session_name();
        if tmux::window_exists(&session, &task_name) {
            println!("Stopping task window...");
            tmux::send_interrupt(&session, &task_name)?;
        }
    }

    // Append TaskReset event (replay will clear state)
    project.append_event(&task_name, &Event::TaskReset { ts: event_timestamp() })?;

    println!("Task '{}' reset to initial state.", task_name);
    println!("Note: Git resources (branch, worktree) are NOT automatically cleaned.");
    println!("Clean up manually if needed:");
    println!("  git worktree remove .wf/worktrees/{} --force", task_name);
    println!("  git branch -D wf/{}", task_name);

    Ok(())
}

/// Internal: called when in_window command exits
pub fn on_exit(task_name: &str, exit_code: i32) -> Result<()> {
    let project = Project::load()?;

    let state = project.replay_task(task_name)?;

    // If task doesn't exist or already not running, nothing to do
    let Some(state) = state else {
        return Ok(());
    };

    if state.status != TaskStatus::Running {
        return Ok(());
    }

    let step_idx = state.current_step;

    // Get session info for logging
    let session = project.session_name();

    // Extract session_id before it's lost (window may be closing)
    let session_id = tmux::extract_session_id(&session, task_name);
    let transcript = session_id.as_ref().and_then(|id| tmux::get_transcript_path(id));

    // Emit OnExit event (replay handles done vs on_exit race)
    project.append_event(task_name, &Event::OnExit {
        ts: event_timestamp(),
        step: step_idx,
        exit_code,
        session_id,
        transcript,
    })?;

    // Replay to get new state after event
    let new_state = project.replay_task(task_name)?;

    if let Some(new_state) = new_state {
        if new_state.status == TaskStatus::Running && new_state.current_step > step_idx {
            println!("Step {} completed successfully.", step_idx + 1);
            continue_execution(&project, task_name)?;
        } else if new_state.status == TaskStatus::Failed {
            eprintln!(
                "Task '{}' failed at step {} (exit code: {}). Use 'wf retry {}' to retry.",
                task_name,
                step_idx + 1,
                exit_code,
                task_name
            );
        }
        // If OnExit was ignored (already handled by AgentReported), do nothing
    }

    Ok(())
}
