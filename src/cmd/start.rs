use anyhow::{bail, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::process::CommandExt as _;
use std::time::Instant;
use uuid::Uuid;

use crate::error::PawlError;
use crate::model::config::Step;
use crate::model::event::event_timestamp;
use crate::model::{Event, TaskStatus};
use crate::util::shell::run_command;
use crate::util::variable::Context;
use super::common::Project;

pub fn run(task_name: &str, reset: bool) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    // Check if task is already running
    if let Some(state) = project.replay_task(&task_name)? {
        if reset {
            // Auto-reset before starting
            project.append_event(&task_name, &Event::TaskReset { ts: event_timestamp() })?;
        } else {
            match state.status {
                TaskStatus::Running => {
                    return Err(PawlError::StateConflict {
                        task: task_name.clone(),
                        status: "running".into(),
                        message: format!("already running at step {}", state.current_step),
                    }.into());
                }
                TaskStatus::Completed => {
                    return Err(PawlError::StateConflict {
                        task: task_name.clone(),
                        status: "completed".into(),
                        message: format!("use 'pawl reset {}' to restart or 'pawl start --reset {}'", task_name, task_name),
                    }.into());
                }
                TaskStatus::Waiting => {
                    let step_name = project.step_name(state.current_step);
                    let reason = state.message.as_deref().unwrap_or("approval");
                    return Err(PawlError::StateConflict {
                        task: task_name.clone(),
                        status: "waiting".into(),
                        message: format!("waiting at step {} ({}) for {}. Use 'pawl done {}' to continue", state.current_step, step_name, reason, task_name),
                    }.into());
                }
                _ => {}
            }
        }
    }

    // Check dependencies
    let blocking = project.check_dependencies(&task_name)?;
    if !blocking.is_empty() {
        return Err(PawlError::Precondition {
            message: format!("Task '{}' is blocked by incomplete dependencies: {}", task_name, blocking.join(", ")),
        }.into());
    }

    // Emit TaskStarted event with run_id
    let run_id = Uuid::new_v4().to_string();
    project.append_event(&task_name, &Event::TaskStarted {
        ts: event_timestamp(),
        run_id,
    })?;

    eprintln!("Starting task: {}", task_name);

    // Execute the workflow
    execute(&project, &task_name)?;

    // Output final state as JSON
    project.output_task_state(&task_name)?;

    Ok(())
}

/// Continue execution from current step (called by pawl done, pawl reset --step, etc.)
pub fn resume_workflow(project: &Project, task_name: &str) -> Result<()> {
    execute(project, task_name)
}

/// Execute workflow steps starting from current_step
fn execute(project: &Project, task_name: &str) -> Result<()> {
    let skip_list: Vec<String> = project
        .task_config(task_name)
        .map(|tc| tc.skip.clone())
        .unwrap_or_default();

    loop {
        // Replay to get current state
        let state = project.replay_task(task_name)?;
        let state = state.expect("Task state missing");
        let step_idx = state.current_step;

        let workflow_len = project.config.workflow.len();

        // Check if we've completed all steps
        if step_idx >= workflow_len {
            eprintln!("Task '{}' completed!", task_name);
            return Ok(());
        }

        let step = &project.config.workflow[step_idx];
        let run_id = &state.run_id;

        // Check if this step should be skipped for this task
        if skip_list.contains(&step.name) {
            project.append_event(task_name, &Event::StepSkipped {
                ts: event_timestamp(),
                step: step_idx,
            })?;
            eprintln!(
                "[{}/{}] {} (skipped)",
                step_idx + 1,
                workflow_len,
                step.name
            );
            continue;
        }

        let mut ctx = project.context_for(task_name, Some(step_idx), run_id);
        let events = project.read_events(task_name)?;
        let (retry_count, last_feedback) = super::common::extract_step_context(&events, step_idx);
        ctx = ctx.var("retry_count", retry_count.to_string());
        if let Some(fb) = &last_feedback {
            ctx = ctx.var("last_verify_output", fb);
        }

        eprintln!(
            "[{}/{}] {}",
            step_idx + 1,
            workflow_len,
            step.name
        );

        // Handle different step types
        if step.is_gate() {
            // Gate step: wait for approval
            project.append_event(task_name, &Event::StepYielded {
                ts: event_timestamp(),
                step: step_idx,
                reason: "gate".to_string(),
            })?;
            eprintln!("  → Waiting for approval. Use 'pawl done {}' to continue.", task_name);
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

            launch_in_viewport(project, task_name, &ctx, step_idx)?;
            return Ok(());
        } else {
            // Normal step: execute synchronously
            let should_continue = execute_step(project, task_name, step, &ctx, &expanded)?;
            if !should_continue {
                return Ok(());
            }
        }
    }
}

// --- Step record ---

pub struct StepRecord {
    pub exit_code: i32,
    pub duration: Option<f64>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

/// Execute a normal (synchronous) step with streaming stdout
fn execute_step(
    project: &Project,
    task_name: &str,
    step: &Step,
    ctx: &Context,
    command: &str,
) -> Result<bool> {
    let state = project.replay_task(task_name)?.expect("Task state missing");
    let step_idx = state.current_step;

    // Set up stream file for live output
    let stream_file = project.stream_file(task_name);
    let streams_dir = stream_file.parent().unwrap();
    fs::create_dir_all(streams_dir)?;
    fs::write(&stream_file, "")?;

    let start_time = Instant::now();
    let env = ctx.to_env_vars();

    let stream_path = stream_file.clone();
    let result = run_command(command, &env, |line| {
        if let Ok(mut f) = OpenOptions::new().append(true).open(&stream_path) {
            let _ = writeln!(f, "{}", line);
        }
    })?;

    let duration = start_time.elapsed().as_secs_f64();

    let record = StepRecord {
        exit_code: result.exit_code,
        duration: Some(duration),
        stdout: Some(result.stdout.clone()),
        stderr: Some(result.stderr.clone()),
    };

    // Clean up stream file before settle_step
    let _ = fs::remove_file(&stream_file);

    if result.success {
        eprintln!("  ✓ Done");
    } else {
        eprintln!("  ✗ Failed (exit code {})", result.exit_code);
        if !result.stderr.is_empty() {
            for line in result.stderr.lines().take(5) {
                eprintln!("    {}", line);
            }
        }
    }

    settle_step(project, task_name, step_idx, step, record)
}

// --- combine | decide | split pipeline ---

#[derive(Debug, PartialEq)]
pub(crate) enum Outcome {
    Success,
    ManualNeeded,
    Failure { feedback: String },
}

#[derive(Debug, PartialEq)]
pub(crate) enum FailPolicy {
    Terminal,
    Retry { can_retry: bool },
    Manual,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Verdict {
    Advance,
    Yield { reason: &'static str },
    Retry,
    Fail,
}

/// Pure decision function: 2 parameters, 6 rules.
pub(crate) fn decide(outcome: Outcome, policy: FailPolicy) -> Verdict {
    match outcome {
        Outcome::Success => Verdict::Advance,
        Outcome::ManualNeeded => Verdict::Yield { reason: "verify_manual" },
        Outcome::Failure { .. } => match policy {
            FailPolicy::Retry { can_retry: true } => Verdict::Retry,
            FailPolicy::Manual => Verdict::Yield { reason: "on_fail_manual" },
            _ => Verdict::Fail,
        },
    }
}

fn derive_fail_policy(project: &Project, task_name: &str, step: &Step, step_idx: usize) -> Result<FailPolicy> {
    match step.on_fail.as_deref() {
        Some("retry") => {
            let events = project.read_events(task_name)?;
            let count = crate::model::event::count_auto_retries(&events, step_idx);
            Ok(FailPolicy::Retry { can_retry: count < step.effective_max_retries() })
        }
        Some("manual") => Ok(FailPolicy::Manual),
        _ => Ok(FailPolicy::Terminal),
    }
}

/// Recording + Routing: first unconditionally record, then route.
fn apply_verdict(
    project: &Project,
    task_name: &str,
    step_idx: usize,
    step: &Step,
    record: StepRecord,
    verdict: &Verdict,
    verify_output: Option<String>,
) -> Result<bool> {
    let success = matches!(verdict, Verdict::Advance | Verdict::Yield { reason: "verify_manual" });

    // Phase 1: Recording — always faithfully record the run result
    project.append_event(task_name, &Event::StepFinished {
        ts: event_timestamp(),
        step: step_idx,
        success,
        exit_code: record.exit_code,
        duration: record.duration,
        stdout: record.stdout,
        stderr: record.stderr,
        verify_output,
    })?;

    // Phase 2: Routing — control flow decision
    match verdict {
        Verdict::Advance => {
            let new_state = project.replay_task(task_name)?.expect("Task state missing");
            if new_state.status == TaskStatus::Completed {
                eprintln!("Task '{}' completed!", task_name);
                return Ok(false);
            }
            Ok(true)
        }
        Verdict::Yield { reason } => {
            project.append_event(task_name, &Event::StepYielded {
                ts: event_timestamp(),
                step: step_idx,
                reason: reason.to_string(),
            })?;
            match *reason {
                "verify_manual" => {
                    eprintln!("  → Waiting for manual verification. Use 'pawl done {}' to approve.", task_name);
                }
                "on_fail_manual" => {
                    eprintln!("  Verify failed. Waiting for manual decision.");
                    eprintln!("  Use 'pawl done {}' to approve or 'pawl reset --step {}' to retry.", task_name, task_name);
                }
                _ => {}
            }
            Ok(false)
        }
        Verdict::Retry => {
            let events = project.read_events(task_name)?;
            let retry_count = crate::model::event::count_auto_retries(&events, step_idx);
            eprintln!("  Verify failed (attempt {}/{}). Auto-retrying...",
                     retry_count + 1, step.effective_max_retries());
            project.append_event(task_name, &Event::StepReset {
                ts: event_timestamp(),
                step: step_idx,
                auto: true,
            })?;
            resume_workflow(project, task_name)?;
            Ok(false)
        }
        Verdict::Fail => {
            eprintln!("  ✗ Failed.");
            Ok(false)
        }
    }
}

/// Unified pipeline: combine → decide → split.
/// Called from execute_step, run_in_viewport, and done.
pub fn settle_step(
    project: &Project,
    task_name: &str,
    step_idx: usize,
    step: &Step,
    record: StepRecord,
) -> Result<bool> {
    // combine: (exit_code, verify) → Outcome
    let (outcome, verify_output) = if record.exit_code == 0 {
        match run_verify(project, task_name, step, step_idx)? {
            VerifyResult::Passed => (Outcome::Success, None),
            VerifyResult::ManualNeeded => (Outcome::ManualNeeded, None),
            VerifyResult::Failed { feedback } => (
                Outcome::Failure { feedback: feedback.clone() },
                Some(feedback),
            ),
        }
    } else {
        (Outcome::Failure { feedback: format!("Exit code: {}", record.exit_code) }, None)
    };

    // derive FailPolicy from Step config + retry state
    let policy = derive_fail_policy(project, task_name, step, step_idx)?;

    // decide
    let verdict = decide(outcome, policy);

    // split: apply verdict
    apply_verdict(project, task_name, step_idx, step, record, &verdict, verify_output)
}

/// Execute an in_viewport step (send to viewport)
fn launch_in_viewport(
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
        eprintln!("  → exec into next in_viewport step");
        let err = std::process::Command::new(&wf_bin)
            .args(["_run", task_name, &step_idx.to_string()])
            .exec();
        bail!("exec failed: {}", err);
    }

    let session = ctx.get("session").unwrap();

    project.viewport.open(task_name, &project.project_root)?;

    eprintln!("  → Sending to {}:{}", session, task_name);
    eprintln!("  → Waiting for 'pawl done {}'", task_name);

    project.viewport.execute(task_name, &run_cmd)?;

    Ok(())
}

// --- Verify helpers ---

#[derive(Debug, PartialEq)]
enum VerifyResult {
    Passed,
    ManualNeeded,
    Failed { feedback: String },
}

/// Run the verify command for a step, if any.
fn run_verify(project: &Project, task_name: &str, step: &Step, step_idx: usize) -> Result<VerifyResult> {
    match &step.verify {
        None => Ok(VerifyResult::Passed),
        Some(v) if v == "manual" => Ok(VerifyResult::ManualNeeded),
        Some(cmd) => {
            let run_id = project.replay_task(task_name)
                .ok()
                .flatten()
                .map(|s| s.run_id)
                .unwrap_or_default();
            let mut ctx = project.context_for(task_name, Some(step_idx), &run_id);
            let events = project.read_events(task_name)?;
            let (retry_count, last_feedback) = super::common::extract_step_context(&events, step_idx);
            ctx = ctx.var("retry_count", retry_count.to_string());
            if let Some(fb) = &last_feedback {
                ctx = ctx.var("last_verify_output", fb);
            }
            let expanded = ctx.expand(cmd);
            let env = ctx.to_env_vars();
            let result = run_command(&expanded, &env, |_| {})?;

            if result.success {
                Ok(VerifyResult::Passed)
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
                Ok(VerifyResult::Failed { feedback })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decide_advance() {
        assert_eq!(decide(Outcome::Success, FailPolicy::Terminal), Verdict::Advance);
    }

    #[test]
    fn test_decide_yield_verify_manual() {
        assert_eq!(decide(Outcome::ManualNeeded, FailPolicy::Terminal), Verdict::Yield { reason: "verify_manual" });
    }

    #[test]
    fn test_decide_failure_terminal() {
        assert_eq!(
            decide(Outcome::Failure { feedback: "bad".into() }, FailPolicy::Terminal),
            Verdict::Fail
        );
    }

    #[test]
    fn test_decide_failure_retry_under_limit() {
        assert_eq!(
            decide(Outcome::Failure { feedback: "bad".into() }, FailPolicy::Retry { can_retry: true }),
            Verdict::Retry
        );
    }

    #[test]
    fn test_decide_failure_retry_at_limit() {
        assert_eq!(
            decide(Outcome::Failure { feedback: "bad".into() }, FailPolicy::Retry { can_retry: false }),
            Verdict::Fail
        );
    }

    #[test]
    fn test_decide_failure_manual() {
        assert_eq!(
            decide(Outcome::Failure { feedback: "bad".into() }, FailPolicy::Manual),
            Verdict::Yield { reason: "on_fail_manual" }
        );
    }
}
