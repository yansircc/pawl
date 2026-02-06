use anyhow::{bail, Result};
use std::time::Instant;

use crate::model::config::Step;
use crate::model::event::event_timestamp;
use crate::model::{Event, TaskStatus};
use crate::util::shell::run_command_with_env;
use crate::util::tmux;
use crate::util::variable::Context;

use super::common::Project;

pub fn run(task_name: &str) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;
    let task_def = project.load_task(&task_name)?;

    // Check if task is already running
    if let Some(state) = project.replay_task(&task_name)? {
        match state.status {
            TaskStatus::Running => {
                bail!("Task '{}' is already running at step {}", task_name, state.current_step);
            }
            TaskStatus::Completed => {
                bail!("Task '{}' is already completed. Use 'wf reset {}' to restart.", task_name, task_name);
            }
            TaskStatus::Waiting => {
                bail!("Task '{}' is waiting. Use 'wf next {}' to continue.", task_name, task_name);
            }
            _ => {}
        }
    }

    // Check dependencies
    let blocking = project.check_dependencies(&task_def)?;
    if !blocking.is_empty() {
        bail!(
            "Task '{}' is blocked by incomplete dependencies: {}",
            task_name,
            blocking.join(", ")
        );
    }

    // Emit TaskStarted event
    project.append_event(&task_name, &Event::TaskStarted { ts: event_timestamp() })?;

    println!("Starting task: {}", task_name);

    // Execute the workflow
    execute(&project, &task_name)?;

    Ok(())
}

/// Continue execution from current step (called by wf next, wf done, etc.)
pub fn continue_execution(project: &Project, task_name: &str) -> Result<()> {
    execute(project, task_name)
}

/// Execute workflow steps starting from current_step
fn execute(project: &Project, task_name: &str) -> Result<()> {
    loop {
        // Replay to get current state
        let state = project.replay_task(task_name)?;
        let state = state.expect("Task state missing");
        let step_idx = state.current_step;

        let workflow_len = project.config.workflow.len();
        let session = project.session_name();

        // Check if we've completed all steps
        if step_idx >= workflow_len {
            println!("Task '{}' completed!", task_name);
            return Ok(());
        }

        let step = project.config.workflow[step_idx].clone();
        let worktree_dir = project.config.worktree_dir.clone();
        let repo_root = project.repo_root.clone();

        let log_file = project.log_file(task_name);
        let task_file = project.task_file(task_name);

        let ctx = Context::new_full(
            task_name,
            &session,
            &repo_root,
            &worktree_dir,
            &step.name,
            step_idx,
            &log_file.to_string_lossy(),
            &task_file.to_string_lossy(),
            &project.config.base_branch,
        );

        println!(
            "[{}/{}] {}",
            step_idx + 1,
            workflow_len,
            step.name
        );

        // Handle different step types
        if step.is_gate() {
            if step.verify_is_human() {
                // Gate with human verify: wait for approval
                project.append_event(task_name, &Event::StepWaiting {
                    ts: event_timestamp(),
                    step: step_idx,
                })?;
                println!("  → Waiting for approval. Use 'wf next {}' or 'wf done {}' to continue.", task_name, task_name);
                return Ok(());
            } else {
                // Plain gate without verify: auto-advance (shouldn't normally happen, but safe)
                project.append_event(task_name, &Event::StepWaiting {
                    ts: event_timestamp(),
                    step: step_idx,
                })?;
                println!("  → Waiting for approval. Use 'wf next {}' to continue.", task_name);
                return Ok(());
            }
        }

        let command = step.run.as_ref().unwrap();
        let expanded = ctx.expand(command);

        if step.in_window {
            // in_window step: emit WindowLaunched and send to tmux
            project.append_event(task_name, &Event::WindowLaunched {
                ts: event_timestamp(),
                step: step_idx,
            })?;

            execute_in_window(project, task_name, &ctx, &expanded)?;
            return Ok(());
        } else {
            // Normal step: execute synchronously
            let should_continue = execute_step(project, task_name, &step, &ctx, &expanded)?;
            if !should_continue {
                return Ok(());
            }
        }
    }
}

/// Execute a normal (synchronous) step
fn execute_step(
    project: &Project,
    task_name: &str,
    step: &Step,
    ctx: &Context,
    command: &str,
) -> Result<bool> {
    let state = project.replay_task(task_name)?.expect("Task state missing");
    let step_idx = state.current_step;

    let start_time = Instant::now();

    let env = ctx.to_env_vars();
    let result = run_command_with_env(command, &env)?;

    let duration = start_time.elapsed().as_secs_f64();

    // Emit CommandExecuted event
    project.append_event(task_name, &Event::CommandExecuted {
        ts: event_timestamp(),
        step: step_idx,
        exit_code: result.exit_code,
        duration,
        stdout: result.stdout.clone(),
        stderr: result.stderr.clone(),
    })?;

    if result.success {
        println!("  ✓ Done");

        // After successful run, check verify
        match run_verify(project, task_name, step, step_idx)? {
            VerifyOutcome::Passed => {
                // Check if all steps completed
                let new_state = project.replay_task(task_name)?.expect("Task state missing");
                if new_state.status == TaskStatus::Completed {
                    println!("Task '{}' completed!", task_name);
                    return Ok(false);
                }
                Ok(true)
            }
            VerifyOutcome::HumanRequired => {
                // verify: "human" — wait for human approval
                project.append_event(task_name, &Event::StepWaiting {
                    ts: event_timestamp(),
                    step: step_idx,
                })?;
                println!("  → Waiting for human verification. Use 'wf done {}' to approve.", task_name);
                Ok(false)
            }
            VerifyOutcome::Failed { feedback } => {
                handle_verify_failure(project, task_name, step_idx, &feedback, step)?;
                Ok(false)
            }
        }
    } else {
        println!("  ✗ Failed (exit code {})", result.exit_code);
        if !result.stderr.is_empty() {
            for line in result.stderr.lines().take(5) {
                println!("    {}", line);
            }
        }
        Ok(false)
    }
}

/// Execute an in_window step (send to tmux)
fn execute_in_window(
    _project: &Project,
    task_name: &str,
    ctx: &Context,
    command: &str,
) -> Result<()> {
    let session = &ctx.session;
    let window = &ctx.window;

    if !tmux::session_exists(session) {
        println!("  Creating session {}...", session);
        tmux::create_session(session, Some(&ctx.repo_root))?;
    }

    if !tmux::window_exists(session, window) {
        println!("  Creating window {}:{}...", session, window);
        tmux::create_window(session, window, Some(&ctx.repo_root))?;
    }

    let work_dir = if std::path::Path::new(&ctx.worktree).exists() {
        &ctx.worktree
    } else {
        &ctx.repo_root
    };

    let wrapped = format!(
        "trap 'cd \"{}\" && wf _on-exit {} $?' EXIT; cd '{}' && {}",
        ctx.repo_root, task_name, work_dir, command
    );

    println!("  → Sending to {}:{}", session, window);
    println!("  → Waiting for 'wf done {}' or 'wf fail {}'", task_name, task_name);

    tmux::send_keys(session, window, &wrapped)?;

    Ok(())
}

// --- Verify helpers ---

pub enum VerifyOutcome {
    Passed,
    Failed { feedback: String },
    HumanRequired,
}

/// Run the verify command for a step, if any.
pub fn run_verify(project: &Project, task_name: &str, step: &Step, step_idx: usize) -> Result<VerifyOutcome> {
    match &step.verify {
        None => Ok(VerifyOutcome::Passed),
        Some(v) if v == "human" => Ok(VerifyOutcome::HumanRequired),
        Some(cmd) => {
            let session = project.session_name();
            let log_file = project.log_file(task_name);
            let task_file = project.task_file(task_name);

            let ctx = Context::new_full(
                task_name,
                &session,
                &project.repo_root,
                &project.config.worktree_dir,
                &step.name,
                step_idx,
                &log_file.to_string_lossy(),
                &task_file.to_string_lossy(),
                &project.config.base_branch,
            );

            let expanded = ctx.expand(cmd);
            let env = ctx.to_env_vars();
            let result = run_command_with_env(&expanded, &env)?;

            if result.success {
                Ok(VerifyOutcome::Passed)
            } else {
                let mut feedback = String::new();
                if !result.stdout.is_empty() {
                    feedback.push_str(&result.stdout);
                }
                if !result.stderr.is_empty() {
                    if !feedback.is_empty() {
                        feedback.push('\n');
                    }
                    feedback.push_str(&result.stderr);
                }
                Ok(VerifyOutcome::Failed { feedback })
            }
        }
    }
}

/// Handle a verify failure: emit VerifyFailed, then apply on_fail strategy.
pub fn handle_verify_failure(
    project: &Project,
    task_name: &str,
    step_idx: usize,
    feedback: &str,
    step: &Step,
) -> Result<()> {
    // Emit VerifyFailed event
    project.append_event(task_name, &Event::VerifyFailed {
        ts: event_timestamp(),
        step: step_idx,
        feedback: feedback.to_string(),
    })?;

    if step.on_fail_retry() {
        let retry_count = count_verify_failures(project, task_name, step_idx)?;
        if retry_count <= step.effective_max_retries() {
            println!("  Verify failed (attempt {}/{}). Auto-retrying...",
                     retry_count, step.effective_max_retries());
            project.append_event(task_name, &Event::StepRetried {
                ts: event_timestamp(),
                step: step_idx,
            })?;
            continue_execution(project, task_name)?;
        } else {
            println!("  Verify failed. Max retries ({}) reached.", step.effective_max_retries());
        }
    } else if step.on_fail_human() {
        project.append_event(task_name, &Event::StepWaiting {
            ts: event_timestamp(),
            step: step_idx,
        })?;
        println!("  Verify failed. Waiting for human decision.");
        println!("  Use 'wf retry {}' to retry or 'wf skip {}' to skip.", task_name, task_name);
    } else {
        // Default: just fail (VerifyFailed already set status to Failed)
        println!("  ✗ Verify failed.");
        if !feedback.is_empty() {
            for line in feedback.lines().take(5) {
                println!("    {}", line);
            }
        }
    }

    Ok(())
}

/// Count VerifyFailed events for a specific step since last TaskStarted/TaskReset.
fn count_verify_failures(project: &Project, task_name: &str, step_idx: usize) -> Result<usize> {
    let events = project.read_events(task_name)?;
    let mut count = 0;

    // Iterate from end, counting VerifyFailed for this step until we hit a boundary
    for event in events.iter().rev() {
        match event {
            Event::TaskStarted { .. } | Event::TaskReset { .. } => break,
            Event::VerifyFailed { step, .. } if *step == step_idx => {
                count += 1;
            }
            _ => {}
        }
    }

    Ok(count)
}
