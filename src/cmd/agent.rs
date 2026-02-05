use anyhow::{bail, Result};
use chrono::Utc;
use std::fs;
use std::io::Write;

use crate::model::{StepStatus, TaskStatus};
use crate::util::shell::run_command_with_env;
use crate::util::tmux::{self, CaptureResult};
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

        // Build context for variable expansion
        let session = project.session_name();
        let ctx = Context::new(
            &task_name,
            &session,
            &project.repo_root,
            &project.config.worktree_dir,
            &step.name,
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

    // Get step name for logging
    let step_name = project.config.workflow[step_idx].name.clone();
    let session = project.session_name();

    // Capture tmux content before marking as done (window may be killed after)
    write_in_window_log(&project, &task_name, step_idx, &step_name, &session, "success");

    // Mark step as success
    {
        let state = project.status.get_mut(&task_name).unwrap();
        state.mark_step(step_idx, StepStatus::Success);
        state.current_step += 1;
        state.message = message.map(|s| s.to_string());
    }
    project.save_status()?;

    // Cleanup tmux window
    cleanup_window(&session, &task_name);

    println!("Step {} marked as done.", step_idx + 1);

    // Continue execution
    continue_execution(&mut project, &task_name)?;

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

    // Get step name for logging
    let step_name = project.config.workflow[step_idx].name.clone();
    let session = project.session_name();

    // Capture tmux content before marking as failed
    write_in_window_log(&project, &task_name, step_idx, &step_name, &session, "failed");

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

    // Get step name for logging
    let step_name = project.config.workflow[step_idx].name.clone();
    let session = project.session_name();

    // Capture tmux content before marking as blocked
    write_in_window_log(&project, &task_name, step_idx, &step_name, &session, "blocked");

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

    println!("Step {} marked as blocked.", step_idx + 1);
    if let Some(msg) = message {
        println!("Reason: {}", msg);
    }
    println!("Resolve the issue and use 'wf next {}' to continue.", task_name);

    Ok(())
}

/// Write log for an in_window step (captures tmux content)
fn write_in_window_log(
    project: &Project,
    task_name: &str,
    step_idx: usize,
    step_name: &str,
    session: &str,
    status: &str,
) {
    let log_dir = project.log_dir(task_name);
    let log_path = project.log_path(task_name, step_idx, step_name);

    // Create log directory if it doesn't exist (best-effort)
    if fs::create_dir_all(&log_dir).is_err() {
        return;
    }

    // Capture tmux content
    let captured_at = Utc::now();
    let content = match tmux::capture_pane(session, task_name, 2000) {
        Ok(CaptureResult::Content(c)) => c,
        Ok(CaptureResult::WindowGone) => "(window already gone)".to_string(),
        Err(_) => "(capture failed)".to_string(),
    };

    let log_content = format!(
        "=== Step {}: {} ===\n\
         Type: in_window\n\
         Captured: {}\n\
         Status: {}\n\
         \n\
         [tmux capture]\n\
         {}\n",
        step_idx + 1,
        step_name,
        captured_at.to_rfc3339(),
        status,
        content.trim_end(),
    );

    // Best-effort write
    if let Ok(mut file) = fs::File::create(&log_path) {
        let _ = file.write_all(log_content.as_bytes());
    }
}

/// Cleanup tmux window after step completion (best-effort)
fn cleanup_window(session: &str, window: &str) {
    // Kill the window - errors are silently ignored
    let _ = tmux::kill_window(session, window);
}
