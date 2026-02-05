use anyhow::{bail, Result};

use crate::model::{StepLog, StepStatus, TaskStatus};
use crate::util::shell::run_command_with_env;
use crate::util::tmux;
use crate::util::variable::Context;

use super::common::Project;
use super::start::continue_execution;

/// Mark current step as done (success)
pub fn done(task_name: &str, message: Option<&str>) -> Result<()> {
    let mut project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let (status, step_idx) = {
        let state = project.status.get(&task_name);
        let Some(state) = state else {
            bail!("Task '{}' not found.", task_name);
        };
        (state.status, state.current_step)
    };

    if status != TaskStatus::Running {
        bail!(
            "Task '{}' is not running (status: {:?}). Cannot mark as done.",
            task_name,
            status
        );
    }

    // Check if step has a stop_hook
    let step = &project.config.workflow[step_idx];
    if let Some(stop_hook) = &step.stop_hook {
        println!("Running stop_hook validation...");

        // Build full context for variable expansion
        let session = project.session_name();
        let log_file = project.log_file(&task_name);
        let task_file = project.task_file(&task_name);

        let ctx = Context::new_full(
            &task_name,
            &session,
            &project.repo_root,
            &project.config.worktree_dir,
            &step.name,
            step_idx,
            &log_file.to_string_lossy(),
            &task_file.to_string_lossy(),
            &project.config.base_branch,
        );

        let expanded = ctx.expand(stop_hook);
        let env = ctx.to_env_vars();
        let result = run_command_with_env(&expanded, &env)?;

        if !result.success {
            // Stop hook failed - reject the done request
            eprintln!("Stop hook validation failed (exit code: {})", result.exit_code);
            if !result.stderr.is_empty() {
                for line in result.stderr.lines().take(10) {
                    eprintln!("  {}", line);
                }
            }
            if !result.stdout.is_empty() {
                for line in result.stdout.lines().take(10) {
                    eprintln!("  {}", line);
                }
            }
            bail!(
                "Cannot mark step as done: stop_hook validation failed.\n\
                 Fix the issues and try 'wf done {}' again.",
                task_name
            );
        }
        println!("Stop hook validation passed.");
    }

    // Get step info for logging
    let session = project.session_name();

    // Extract session_id before cleanup (must happen while window still exists)
    let session_id = tmux::extract_session_id(&session, &task_name);
    let transcript = session_id.as_ref().and_then(|id| tmux::get_transcript_path(id));

    // Write metadata log
    let log_entry = StepLog::InWindow {
        step: step_idx,
        session_id,
        transcript,
        status: "success".to_string(),
    };
    let _ = project.append_log(&task_name, &log_entry);

    // Mark step as success
    {
        let state = project.status.get_mut(&task_name).unwrap();
        state.mark_step(step_idx, StepStatus::Success);
        state.current_step += 1;
        state.message = message.map(|s| s.to_string());
    }
    project.save_status()?;

    println!("Step {} marked as done.", step_idx + 1);

    // Continue execution first, then cleanup window
    // This ensures subsequent steps can execute before we destroy the current shell
    continue_execution(&mut project, &task_name)?;

    // Cleanup tmux window after execution completes
    // Note: If continue_execution stopped at another in_window step, the window
    // might be reused. cleanup_window is best-effort and ignores errors.
    cleanup_window(&session, &task_name);

    Ok(())
}

/// Mark current step as failed
pub fn fail(task_name: &str, message: Option<&str>) -> Result<()> {
    let mut project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let (status, step_idx) = {
        let state = project.status.get(&task_name);
        let Some(state) = state else {
            bail!("Task '{}' not found.", task_name);
        };
        (state.status, state.current_step)
    };

    if status != TaskStatus::Running {
        bail!(
            "Task '{}' is not running (status: {:?}). Cannot mark as failed.",
            task_name,
            status
        );
    }

    // Get session info for logging
    let session = project.session_name();

    // Extract session_id before cleanup (must happen while window still exists)
    let session_id = tmux::extract_session_id(&session, &task_name);
    let transcript = session_id.as_ref().and_then(|id| tmux::get_transcript_path(id));

    // Write metadata log
    let log_entry = StepLog::InWindow {
        step: step_idx,
        session_id,
        transcript,
        status: "failed".to_string(),
    };
    let _ = project.append_log(&task_name, &log_entry);

    // Mark step as failed
    {
        let state = project.status.get_mut(&task_name).unwrap();
        state.mark_step(step_idx, StepStatus::Failed);
        state.status = TaskStatus::Failed;
        state.message = message.map(|s| s.to_string());
        state.touch();
    }
    project.save_status()?;

    // Cleanup tmux window
    cleanup_window(&session, &task_name);

    // Fire hooks
    project.fire_hook("step.failed", &task_name);
    project.fire_hook("task.failed", &task_name);

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

    let (status, step_idx) = {
        let state = project.status.get(&task_name);
        let Some(state) = state else {
            bail!("Task '{}' not found.", task_name);
        };
        (state.status, state.current_step)
    };

    if status != TaskStatus::Running {
        bail!(
            "Task '{}' is not running (status: {:?}). Cannot mark as blocked.",
            task_name,
            status
        );
    }

    // Get session info for logging
    let session = project.session_name();

    // Extract session_id before cleanup (must happen while window still exists)
    let session_id = tmux::extract_session_id(&session, &task_name);
    let transcript = session_id.as_ref().and_then(|id| tmux::get_transcript_path(id));

    // Write metadata log
    let log_entry = StepLog::InWindow {
        step: step_idx,
        session_id,
        transcript,
        status: "blocked".to_string(),
    };
    let _ = project.append_log(&task_name, &log_entry);

    // Mark step as blocked
    {
        let state = project.status.get_mut(&task_name).unwrap();
        state.mark_step(step_idx, StepStatus::Blocked);
        state.status = TaskStatus::Waiting;
        state.message = message.map(|s| s.to_string());
        state.touch();
    }
    project.save_status()?;

    // Cleanup tmux window
    cleanup_window(&session, &task_name);

    // Fire hook
    project.fire_hook("step.blocked", &task_name);

    println!("Step {} marked as blocked.", step_idx + 1);
    if let Some(msg) = message {
        println!("Reason: {}", msg);
    }
    println!("Resolve the issue and use 'wf next {}' to continue.", task_name);

    Ok(())
}

/// Cleanup tmux window after step completion (best-effort)
fn cleanup_window(session: &str, window: &str) {
    // Kill the window - errors are silently ignored
    let _ = tmux::kill_window(session, window);
}
