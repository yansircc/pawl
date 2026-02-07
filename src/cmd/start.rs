use anyhow::{bail, Result};
use std::time::Instant;

use crate::model::config::Step;
use crate::model::event::event_timestamp;
use crate::model::{Event, TaskStatus};
use crate::util::shell::run_command_with_env;
use crate::util::tmux;
use crate::util::variable::Context;

use super::common::Project;

pub fn run(task_name: &str, reset: bool) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;
    let task_def = project.load_task(&task_name)?;

    // Check if task is already running
    if let Some(state) = project.replay_task(&task_name)? {
        if reset {
            // Auto-reset before starting
            project.append_event(&task_name, &Event::TaskReset { ts: event_timestamp() })?;
        } else {
            match state.status {
                TaskStatus::Running => {
                    bail!("Task '{}' is already running at step {}", task_name, state.current_step + 1);
                }
                TaskStatus::Completed => {
                    bail!("Task '{}' is already completed. Use 'wf reset {}' to restart or 'wf start --reset {}'.", task_name, task_name, task_name);
                }
                TaskStatus::Waiting => {
                    bail!("Task '{}' is waiting. Use 'wf done {}' to continue.", task_name, task_name);
                }
                _ => {}
            }
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

/// Continue execution from current step (called by wf done, wf reset --step, etc.)
pub fn continue_execution(project: &Project, task_name: &str) -> Result<()> {
    execute(project, task_name)
}

/// Execute workflow steps starting from current_step
fn execute(project: &Project, task_name: &str) -> Result<()> {
    let task_def = project.load_task(task_name)?;

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

        let step = &project.config.workflow[step_idx];
        let worktree_dir = &project.config.worktree_dir;
        let repo_root = &project.repo_root;

        // Check if this step should be skipped for this task
        if task_def.skip.contains(&step.name) {
            project.append_event(task_name, &Event::StepSkipped {
                ts: event_timestamp(),
                step: step_idx,
            })?;
            println!(
                "[{}/{}] {} (skipped)",
                step_idx + 1,
                workflow_len,
                step.name
            );
            continue;
        }

        let log_file = project.log_file(task_name);
        let task_file = project.task_file(task_name);

        let ctx = Context::new(
            task_name,
            &session,
            repo_root,
            worktree_dir,
            &step.name,
            &project.config.base_branch,
            Some(step_idx),
            Some(&log_file.to_string_lossy()),
            Some(&task_file.to_string_lossy()),
        );

        println!(
            "[{}/{}] {}",
            step_idx + 1,
            workflow_len,
            step.name
        );

        // Handle different step types
        if step.is_gate() {
            // Gate step: wait for approval
            project.append_event(task_name, &Event::StepWaiting {
                ts: event_timestamp(),
                step: step_idx,
                reason: "gate".to_string(),
            })?;
            println!("  → Waiting for approval. Use 'wf done {}' to continue.", task_name);
            return Ok(());
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

pub struct RunOutput {
    pub duration: Option<f64>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
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

    let run_output = RunOutput {
        duration: Some(duration),
        stdout: Some(result.stdout.clone()),
        stderr: Some(result.stderr.clone()),
    };

    if result.success {
        println!("  ✓ Done");
    } else {
        println!("  ✗ Failed (exit code {})", result.exit_code);
        if !result.stderr.is_empty() {
            for line in result.stderr.lines().take(5) {
                println!("    {}", line);
            }
        }
    }

    handle_step_completion(project, task_name, step_idx, result.exit_code, step, run_output)
}

/// Unified pipeline: handles verify + on_fail after any step completion.
/// Called from execute_step, on_exit, and done.
/// StepCompleted is emitted INSIDE this function (after verify is resolved).
pub fn handle_step_completion(
    project: &Project,
    task_name: &str,
    step_idx: usize,
    exit_code: i32,
    step: &Step,
    run_output: RunOutput,
) -> Result<bool> {
    if exit_code != 0 {
        // Step failed — emit StepCompleted with failure, then apply on_fail
        project.append_event(task_name, &Event::StepCompleted {
            ts: event_timestamp(),
            step: step_idx,
            exit_code,
            duration: run_output.duration,
            stdout: run_output.stdout,
            stderr: run_output.stderr,
        })?;
        let feedback = format!("Exit code: {}", exit_code);
        return apply_on_fail(project, task_name, step_idx, &feedback, step);
    }

    // Step succeeded — run verify if configured
    match run_verify(project, task_name, step, step_idx)? {
        VerifyOutcome::Passed => {
            // Verify passed (or no verify) — emit StepCompleted(0)
            project.append_event(task_name, &Event::StepCompleted {
                ts: event_timestamp(),
                step: step_idx,
                exit_code: 0,
                duration: run_output.duration,
                stdout: run_output.stdout,
                stderr: run_output.stderr,
            })?;
            // Check if all steps completed
            let new_state = project.replay_task(task_name)?.expect("Task state missing");
            if new_state.status == TaskStatus::Completed {
                println!("Task '{}' completed!", task_name);
                return Ok(false);
            }
            Ok(true)
        }
        VerifyOutcome::HumanRequired => {
            // verify:human — emit StepCompleted(0) then StepWaiting
            project.append_event(task_name, &Event::StepCompleted {
                ts: event_timestamp(),
                step: step_idx,
                exit_code: 0,
                duration: run_output.duration,
                stdout: run_output.stdout,
                stderr: run_output.stderr,
            })?;
            emit_waiting(project, task_name, step_idx, "verify_human",
                &format!("  → Waiting for human verification. Use 'wf done {}' to approve.", task_name))
        }
        VerifyOutcome::Failed { feedback } => {
            // Verify command failed — emit StepCompleted(1, stderr=feedback)
            project.append_event(task_name, &Event::StepCompleted {
                ts: event_timestamp(),
                step: step_idx,
                exit_code: 1,
                duration: run_output.duration,
                stdout: None,
                stderr: Some(feedback.clone()),
            })?;
            apply_on_fail(project, task_name, step_idx, &feedback, step)
        }
    }
}

/// Apply on_fail strategy after a failure (verify failure or step failure).
/// Returns Ok(false) to stop the execution loop.
fn apply_on_fail(
    project: &Project,
    task_name: &str,
    step_idx: usize,
    feedback: &str,
    step: &Step,
) -> Result<bool> {
    if step.on_fail_retry() {
        let retry_count = count_auto_retries(project, task_name, step_idx)?;
        if retry_count < step.effective_max_retries() {
            println!("  Verify failed (attempt {}/{}). Auto-retrying...",
                     retry_count + 1, step.effective_max_retries());
            project.append_event(task_name, &Event::StepReset {
                ts: event_timestamp(),
                step: step_idx,
                auto: true,
            })?;
            continue_execution(project, task_name)?;
            return Ok(false);
        } else {
            println!("  Verify failed. Max retries ({}) reached.", step.effective_max_retries());
        }
    } else if step.on_fail_human() {
        return emit_waiting(project, task_name, step_idx, "on_fail_human",
            &format!("  Verify failed. Waiting for human decision.\n  Use 'wf done {}' to approve or 'wf reset --step {}' to retry.", task_name, task_name));
    } else {
        // Default: just fail
        println!("  ✗ Failed.");
        if !feedback.is_empty() {
            for line in feedback.lines().take(5) {
                println!("    {}", line);
            }
        }
    }

    Ok(false)
}

/// Emit a StepWaiting event and print a message. Returns Ok(false) to stop the execution loop.
fn emit_waiting(project: &Project, task_name: &str, step_idx: usize, reason: &str, message: &str) -> Result<bool> {
    project.append_event(task_name, &Event::StepWaiting {
        ts: event_timestamp(),
        step: step_idx,
        reason: reason.to_string(),
    })?;
    println!("{}", message);
    Ok(false)
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
        "trap 'cd \"{}\" && wf _on-exit {} $?' EXIT; cd '{}' && {}; exit $?",
        ctx.repo_root, task_name, work_dir, command
    );

    println!("  → Sending to {}:{}", session, window);
    println!("  → Waiting for 'wf done {}'", task_name);

    tmux::send_keys(session, window, &wrapped)?;

    Ok(())
}

// --- Verify helpers ---

enum VerifyOutcome {
    Passed,
    HumanRequired,
    Failed { feedback: String },
}

/// Run the verify command for a step, if any.
/// Pure function — no event emission (side-effect-free).
fn run_verify(project: &Project, task_name: &str, step: &Step, step_idx: usize) -> Result<VerifyOutcome> {
    match &step.verify {
        None => Ok(VerifyOutcome::Passed),
        Some(v) if v == "human" => Ok(VerifyOutcome::HumanRequired),
        Some(cmd) => {
            let session = project.session_name();
            let log_file = project.log_file(task_name);
            let task_file = project.task_file(task_name);

            let ctx = Context::new(
                task_name,
                &session,
                &project.repo_root,
                &project.config.worktree_dir,
                &step.name,
                &project.config.base_branch,
                Some(step_idx),
                Some(&log_file.to_string_lossy()),
                Some(&task_file.to_string_lossy()),
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

/// Count auto-retries for a specific step since last TaskStarted/TaskReset(manual).
fn count_auto_retries(project: &Project, task_name: &str, step_idx: usize) -> Result<usize> {
    let events = project.read_events(task_name)?;
    let mut count = 0;

    for event in events.iter().rev() {
        match event {
            Event::TaskStarted { .. } | Event::TaskReset { .. } => break,
            // Manual reset (wf reset --step) resets the counter
            Event::StepReset { step, auto: false, .. } if *step == step_idx => break,
            // Auto retry counts toward the limit
            Event::StepReset { step, auto: true, .. } if *step == step_idx => {
                count += 1;
            }
            _ => {}
        }
    }

    Ok(count)
}
