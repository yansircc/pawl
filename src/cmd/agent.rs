use anyhow::{bail, Result};

use crate::model::event::event_timestamp;
use crate::model::{AgentResult, Event, TaskStatus};
use crate::util::shell::run_command_with_env;
use crate::util::tmux;
use crate::util::variable::Context;

use super::common::Project;
use super::start::continue_execution;

/// Mark current step as done (success)
pub fn done(task_name: &str, message: Option<&str>) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.replay_task(&task_name)?;
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

    // Check if step has a stop_hook
    let step = &project.config.workflow[step_idx];
    if let Some(stop_hook) = &step.stop_hook {
        println!("Running stop_hook validation...");

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

    if state.status != TaskStatus::Running {
        bail!(
            "Task '{}' is not running (status: {:?}). Cannot mark as failed.",
            task_name,
            state.status
        );
    }

    let step_idx = state.current_step;

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
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    let state = project.replay_task(&task_name)?;
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

    // Extract session_id before cleanup
    let session = project.session_name();
    let session_id = tmux::extract_session_id(&session, &task_name);
    let transcript = session_id.as_ref().and_then(|id| tmux::get_transcript_path(id));

    // Emit AgentReported Blocked event
    project.append_event(&task_name, &Event::AgentReported {
        ts: event_timestamp(),
        step: step_idx,
        result: AgentResult::Blocked,
        session_id,
        transcript,
        message: message.map(|s| s.to_string()),
    })?;

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
    let _ = tmux::kill_window(session, window);
}
