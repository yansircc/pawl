use anyhow::{bail, Result};
use chrono::Utc;
use serde_json::json;
use std::fs;
use std::io::Write;

use crate::model::{StepStatus, TaskStatus};
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

        // Build full context for variable expansion (including log paths)
        let session = project.session_name();
        let log_dir = project.log_dir(&task_name);
        let log_path = project.log_path(&task_name, step_idx, &step.name);
        let prev_log = project.prev_log_path(&task_name, step_idx);
        let prev_log_str = prev_log.as_ref().map(|p| p.to_string_lossy().to_string());

        let ctx = Context::new_full(
            &task_name,
            &session,
            &project.repo_root,
            &project.config.worktree_dir,
            &step.name,
            step_idx,
            &log_dir.to_string_lossy(),
            &log_path.to_string_lossy(),
            prev_log_str.as_deref(),
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
    let step = &project.config.workflow[step_idx];
    let step_name = step.name.clone();
    let command = step.run.clone().unwrap_or_default();
    let session = project.session_name();

    // Write metadata log
    write_in_window_log(&project, &task_name, step_idx, &step_name, &command, "success");

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

    // Get step info for logging
    let step = &project.config.workflow[step_idx];
    let step_name = step.name.clone();
    let command = step.run.clone().unwrap_or_default();
    let session = project.session_name();

    // Write metadata log
    write_in_window_log(&project, &task_name, step_idx, &step_name, &command, "failed");

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

    // Get step info for logging
    let step = &project.config.workflow[step_idx];
    let step_name = step.name.clone();
    let command = step.run.clone().unwrap_or_default();
    let session = project.session_name();

    // Write metadata log
    write_in_window_log(&project, &task_name, step_idx, &step_name, &command, "blocked");

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

/// Write JSON metadata log for an in_window step
fn write_in_window_log(
    project: &Project,
    task_name: &str,
    step_idx: usize,
    step_name: &str,
    command: &str,
    status: &str,
) {
    let log_dir = project.log_dir(task_name);
    let log_path = project.log_path(task_name, step_idx, step_name);

    // Create log directory if it doesn't exist (best-effort)
    if fs::create_dir_all(&log_dir).is_err() {
        return;
    }

    let completed_at = Utc::now();

    // Write JSON metadata only
    let log_data = json!({
        "step": step_idx + 1,
        "name": step_name,
        "type": "in_window",
        "command": command,
        "completed": completed_at.to_rfc3339(),
        "status": status
    });

    // Best-effort write
    if let Ok(mut file) = fs::File::create(&log_path) {
        let _ = file.write_all(serde_json::to_string_pretty(&log_data).unwrap_or_default().as_bytes());
    }
}

/// Cleanup tmux window after step completion (best-effort)
fn cleanup_window(session: &str, window: &str) {
    // Kill the window - errors are silently ignored
    let _ = tmux::kill_window(session, window);
}
