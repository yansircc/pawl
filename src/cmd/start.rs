use anyhow::{bail, Result};
use std::os::unix::process::CommandExt as _;
use std::time::Instant;

use crate::model::config::Step;
use crate::model::event::event_timestamp;
use crate::model::{Event, TaskStatus};
use crate::util::shell::run_command_with_env;
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
                    bail!("Task '{}' is already completed. Use 'pawl reset {}' to restart or 'pawl start --reset {}'.", task_name, task_name, task_name);
                }
                TaskStatus::Waiting => {
                    let step_name = if state.current_step < project.config.workflow.len() {
                        &project.config.workflow[state.current_step].name
                    } else {
                        "unknown"
                    };
                    let reason = state.message.as_deref().unwrap_or("approval");
                    bail!(
                        "Task '{}' is waiting at step {} ({}) for {}. Use 'pawl done {}' to continue.",
                        task_name, state.current_step + 1, step_name, reason, task_name
                    );
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

/// Continue execution from current step (called by pawl done, pawl reset --step, etc.)
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
            &project.config.claude_command,
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
            println!("  → Waiting for approval. Use 'pawl done {}' to continue.", task_name);
            return Ok(());
        }

        let command = step.run.as_ref().unwrap();
        let expanded = ctx.expand(command);

        if step.in_viewport {
            // in_viewport step: emit ViewportLaunched and send to viewport
            project.append_event(task_name, &Event::ViewportLaunched {
                ts: event_timestamp(),
                step: step_idx,
            })?;

            execute_in_viewport(project, task_name, &ctx, step_idx)?;
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

// --- resolve/dispatch pipeline ---

#[derive(Debug, PartialEq)]
pub(crate) enum Action {
    Advance,
    YieldVerifyHuman,
    Retry { exit_code: i32, feedback: String },
    YieldOnFailHuman { exit_code: i32, feedback: String },
    Fail { exit_code: i32, feedback: String },
}

/// Pure decision function: given step outcome, determine the next action.
pub(crate) fn resolve(
    exit_code: i32,
    verify_outcome: VerifyOutcome,
    on_fail: Option<&str>,
    retry_count: usize,
    max_retries: usize,
) -> Action {
    if exit_code == 0 {
        match verify_outcome {
            VerifyOutcome::Passed => Action::Advance,
            VerifyOutcome::HumanRequired => Action::YieldVerifyHuman,
            VerifyOutcome::Failed { feedback } => {
                resolve_failure(1, feedback, on_fail, retry_count, max_retries)
            }
        }
    } else {
        let feedback = format!("Exit code: {}", exit_code);
        resolve_failure(exit_code, feedback, on_fail, retry_count, max_retries)
    }
}

fn resolve_failure(
    exit_code: i32,
    feedback: String,
    on_fail: Option<&str>,
    retry_count: usize,
    max_retries: usize,
) -> Action {
    match on_fail {
        Some("retry") if retry_count < max_retries => {
            Action::Retry { exit_code, feedback }
        }
        Some("human") => {
            Action::YieldOnFailHuman { exit_code, feedback }
        }
        _ => Action::Fail { exit_code, feedback },
    }
}

/// IO function: execute the decided action (emit events, print messages).
fn dispatch(
    project: &Project,
    task_name: &str,
    step_idx: usize,
    step: &Step,
    run_output: RunOutput,
    action: Action,
) -> Result<bool> {
    match action {
        Action::Advance => {
            project.append_event(task_name, &Event::StepCompleted {
                ts: event_timestamp(),
                step: step_idx,
                exit_code: 0,
                duration: run_output.duration,
                stdout: run_output.stdout,
                stderr: run_output.stderr,
            })?;
            let new_state = project.replay_task(task_name)?.expect("Task state missing");
            if new_state.status == TaskStatus::Completed {
                println!("Task '{}' completed!", task_name);
                return Ok(false);
            }
            Ok(true)
        }
        Action::YieldVerifyHuman => {
            project.append_event(task_name, &Event::StepCompleted {
                ts: event_timestamp(),
                step: step_idx,
                exit_code: 0,
                duration: run_output.duration,
                stdout: run_output.stdout,
                stderr: run_output.stderr,
            })?;
            emit_waiting(project, task_name, step_idx, "verify_human",
                &format!("  → Waiting for human verification. Use 'pawl done {}' to approve.", task_name))
        }
        Action::Retry { exit_code, feedback } => {
            emit_step_completed_for_failure(project, task_name, step_idx, exit_code, &feedback, &run_output)?;
            let events = project.read_events(task_name)?;
            let retry_count = crate::model::event::count_auto_retries(&events, step_idx);
            println!("  Verify failed (attempt {}/{}). Auto-retrying...",
                     retry_count + 1, step.effective_max_retries());
            project.append_event(task_name, &Event::StepReset {
                ts: event_timestamp(),
                step: step_idx,
                auto: true,
            })?;
            continue_execution(project, task_name)?;
            Ok(false)
        }
        Action::YieldOnFailHuman { exit_code, feedback } => {
            emit_step_completed_for_failure(project, task_name, step_idx, exit_code, &feedback, &run_output)?;
            emit_waiting(project, task_name, step_idx, "on_fail_human",
                &format!("  Verify failed. Waiting for human decision.\n  Use 'pawl done {}' to approve or 'pawl reset --step {}' to retry.", task_name, task_name))
        }
        Action::Fail { exit_code, feedback } => {
            emit_step_completed_for_failure(project, task_name, step_idx, exit_code, &feedback, &run_output)?;
            println!("  ✗ Failed.");
            if !feedback.is_empty() {
                for line in feedback.lines().take(5) {
                    println!("    {}", line);
                }
            }
            Ok(false)
        }
    }
}

/// Emit StepCompleted for failure cases.
/// For run failures (exit_code from the run command), use original run_output.
/// For verify failures (exit_code == 1, synthetic), use stdout=None, stderr=Some(feedback).
fn emit_step_completed_for_failure(
    project: &Project,
    task_name: &str,
    step_idx: usize,
    exit_code: i32,
    feedback: &str,
    run_output: &RunOutput,
) -> Result<()> {
    let is_verify_failure = feedback != format!("Exit code: {}", exit_code).as_str();
    if is_verify_failure {
        project.append_event(task_name, &Event::StepCompleted {
            ts: event_timestamp(),
            step: step_idx,
            exit_code,
            duration: run_output.duration,
            stdout: None,
            stderr: Some(feedback.to_string()),
        })?;
    } else {
        project.append_event(task_name, &Event::StepCompleted {
            ts: event_timestamp(),
            step: step_idx,
            exit_code,
            duration: run_output.duration,
            stdout: run_output.stdout.clone(),
            stderr: run_output.stderr.clone(),
        })?;
    }
    Ok(())
}

/// Unified pipeline: handles verify + on_fail after any step completion.
/// Called from execute_step, run_in_viewport, and done.
/// StepCompleted is emitted INSIDE this function (after verify is resolved).
pub fn handle_step_completion(
    project: &Project,
    task_name: &str,
    step_idx: usize,
    exit_code: i32,
    step: &Step,
    run_output: RunOutput,
) -> Result<bool> {
    let verify_outcome = if exit_code == 0 {
        run_verify(project, task_name, step, step_idx)?
    } else {
        VerifyOutcome::Passed
    };

    let events = project.read_events(task_name)?;
    let retry_count = crate::model::event::count_auto_retries(&events, step_idx);

    let action = resolve(
        exit_code,
        verify_outcome,
        step.on_fail.as_deref(),
        retry_count,
        step.effective_max_retries(),
    );

    dispatch(project, task_name, step_idx, step, run_output, action)
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

/// Execute an in_viewport step (send to viewport)
fn execute_in_viewport(
    project: &Project,
    task_name: &str,
    ctx: &Context,
    step_idx: usize,
) -> Result<()> {
    let wf_bin = std::env::current_exe()?.to_string_lossy().to_string();
    let run_cmd = format!("{} _run {} {}", wf_bin, task_name, step_idx);

    // If already running inside a viewport (consecutive in_viewport steps),
    // exec directly instead of sending via viewport
    if std::env::var("PAWL_IN_VIEWPORT").ok().as_deref() == Some(task_name) {
        println!("  → exec into next in_viewport step");
        let err = std::process::Command::new(&wf_bin)
            .args(["_run", task_name, &step_idx.to_string()])
            .exec();
        bail!("exec failed: {}", err);
    }

    let session = &ctx.session;

    project.viewport.open(task_name, &ctx.repo_root)?;

    println!("  → Sending to {}:{}", session, task_name);
    println!("  → Waiting for 'pawl done {}'", task_name);

    project.viewport.send(task_name, &run_cmd)?;

    Ok(())
}

// --- Verify helpers ---

#[derive(Debug, PartialEq)]
pub(crate) enum VerifyOutcome {
    Passed,
    HumanRequired,
    Failed { feedback: String },
}

/// Run the verify command for a step, if any.
/// Pure function — no event emission (side-effect-free).
pub(crate) fn run_verify(project: &Project, task_name: &str, step: &Step, step_idx: usize) -> Result<VerifyOutcome> {
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
                &project.config.claude_command,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_advance() {
        assert_eq!(resolve(0, VerifyOutcome::Passed, None, 0, 3), Action::Advance);
    }

    #[test]
    fn test_resolve_yield_verify_human() {
        assert_eq!(resolve(0, VerifyOutcome::HumanRequired, None, 0, 3), Action::YieldVerifyHuman);
    }

    #[test]
    fn test_resolve_verify_failed_no_on_fail() {
        assert_eq!(
            resolve(0, VerifyOutcome::Failed { feedback: "bad".into() }, None, 0, 3),
            Action::Fail { exit_code: 1, feedback: "bad".into() }
        );
    }

    #[test]
    fn test_resolve_verify_failed_retry_under_limit() {
        assert_eq!(
            resolve(0, VerifyOutcome::Failed { feedback: "bad".into() }, Some("retry"), 1, 3),
            Action::Retry { exit_code: 1, feedback: "bad".into() }
        );
    }

    #[test]
    fn test_resolve_verify_failed_retry_at_limit() {
        assert_eq!(
            resolve(0, VerifyOutcome::Failed { feedback: "bad".into() }, Some("retry"), 3, 3),
            Action::Fail { exit_code: 1, feedback: "bad".into() }
        );
    }

    #[test]
    fn test_resolve_verify_failed_human() {
        assert_eq!(
            resolve(0, VerifyOutcome::Failed { feedback: "bad".into() }, Some("human"), 0, 3),
            Action::YieldOnFailHuman { exit_code: 1, feedback: "bad".into() }
        );
    }

    #[test]
    fn test_resolve_run_failed_no_on_fail() {
        assert_eq!(
            resolve(42, VerifyOutcome::Passed, None, 0, 3),
            Action::Fail { exit_code: 42, feedback: "Exit code: 42".into() }
        );
    }
}
