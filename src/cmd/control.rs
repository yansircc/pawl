use anyhow::{bail, Result};

use crate::model::event::event_timestamp;
use crate::model::{Event, TaskStatus};
use crate::util::tmux;

use super::common::Project;
use super::start;
use super::start::{continue_execution, RunOutput};

/// Stop the current task
pub fn stop(task_name: &str) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.replay_task(&task_name)?;
    let Some(state) = state else {
        bail!("Task '{}' has not been started. Use 'wf start {}' to begin.", task_name, task_name);
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
    println!("Use 'wf reset --step {}' to retry or 'wf reset {}' to restart.", task_name, task_name);

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
            bail!("Task '{}' has not been started.", task_name);
        };

        let step_idx = state.current_step;
        if step_idx >= project.config.workflow.len() {
            bail!("Task '{}' is already completed. Use 'wf reset {}' for full reset.", task_name, task_name);
        }

        match state.status {
            TaskStatus::Failed | TaskStatus::Stopped | TaskStatus::Waiting => {}
            TaskStatus::Running => {
                bail!("Task '{}' is already running.", task_name);
            }
            TaskStatus::Completed => {
                bail!("Task '{}' is completed. Use 'wf reset {}' for full reset.", task_name, task_name);
            }
            TaskStatus::Pending => {
                bail!("Task '{}' has not been started. Use 'wf start {}'", task_name, task_name);
            }
        }

        project.append_event(&task_name, &Event::StepReset {
            ts: event_timestamp(),
            step: step_idx,
            auto: false,
        })?;

        let step_name = project.config.workflow[step_idx].name.clone();
        println!("Reset step {}: {}", step_idx + 1, step_name);
        continue_execution(&project, &task_name)?;
    } else {
        // Full task reset
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

        project.append_event(&task_name, &Event::TaskReset { ts: event_timestamp() })?;

        println!("Task '{}' reset to initial state.", task_name);
        println!("Note: Git resources (branch, worktree) are NOT automatically cleaned.");
        println!("Clean up manually if needed:");
        println!("  git worktree remove .wf/worktrees/{} --force", task_name);
        println!("  git branch -D wf/{}", task_name);
    }

    Ok(())
}

/// Internal: called when in_window command exits
pub fn on_exit(task_name: &str, exit_code: i32) -> Result<()> {
    let project = Project::load()?;

    let state = project.replay_task(task_name)?;

    let Some(state) = state else {
        return Ok(());
    };

    if state.status != TaskStatus::Running {
        return Ok(());
    }

    let step_idx = state.current_step;
    let step = &project.config.workflow[step_idx];

    // P12 fix: if exit_code=0 but window is gone, it was killed by signal (SIGHUP from kill-window)
    if exit_code == 0 && step.in_window {
        let session = project.session_name();
        if !tmux::window_exists(&session, task_name) {
            project.append_event(task_name, &Event::WindowLost {
                ts: event_timestamp(),
                step: step_idx,
            })?;
            eprintln!("Task '{}' window lost at step {} (killed by signal).", task_name, step_idx + 1);
            return Ok(());
        }
    }

    // Use unified pipeline (StepCompleted emitted inside)
    let run_output = RunOutput {
        duration: None,
        stdout: None,
        stderr: None,
    };
    let step = step.clone();
    match start::handle_step_completion(&project, task_name, step_idx, exit_code, &step, run_output)? {
        true => {
            // Pipeline says continue — run next steps
            continue_execution(&project, task_name)?;
        }
        false => {
            if exit_code != 0 {
                eprintln!(
                    "Task '{}' failed at step {} (exit code: {}). Use 'wf reset --step {}' to retry.",
                    task_name,
                    step_idx + 1,
                    exit_code,
                    task_name
                );
            }
        }
    }

    Ok(())
}
