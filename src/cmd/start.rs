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
                    let step_name = project.step_name(state.current_step);
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
pub fn resume_workflow(project: &Project, task_name: &str) -> Result<()> {
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

        // Check if we've completed all steps
        if step_idx >= workflow_len {
            println!("Task '{}' completed!", task_name);
            return Ok(());
        }

        let step = &project.config.workflow[step_idx];

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

        let ctx = project.context_for(task_name, Some(step_idx));

        println!(
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

    let record = StepRecord {
        exit_code: result.exit_code,
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

    settle_step(project, task_name, step_idx, step, record)
}

// --- combine | decide | split pipeline ---

#[derive(Debug, PartialEq)]
pub(crate) enum Outcome {
    Success,
    HumanNeeded,
    Failure { feedback: String },
}

#[derive(Debug, PartialEq)]
pub(crate) enum FailPolicy {
    Terminal,
    Retry { can_retry: bool },
    Human,
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
        Outcome::HumanNeeded => Verdict::Yield { reason: "verify_human" },
        Outcome::Failure { .. } => match policy {
            FailPolicy::Retry { can_retry: true } => Verdict::Retry,
            FailPolicy::Human => Verdict::Yield { reason: "on_fail_human" },
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
        Some("human") => Ok(FailPolicy::Human),
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
    let success = matches!(verdict, Verdict::Advance | Verdict::Yield { reason: "verify_human" });

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
                println!("Task '{}' completed!", task_name);
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
                "verify_human" => {
                    println!("  → Waiting for human verification. Use 'pawl done {}' to approve.", task_name);
                }
                "on_fail_human" => {
                    println!("  Verify failed. Waiting for human decision.");
                    println!("  Use 'pawl done {}' to approve or 'pawl reset --step {}' to retry.", task_name, task_name);
                }
                _ => {}
            }
            Ok(false)
        }
        Verdict::Retry => {
            let events = project.read_events(task_name)?;
            let retry_count = crate::model::event::count_auto_retries(&events, step_idx);
            println!("  Verify failed (attempt {}/{}). Auto-retrying...",
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
            println!("  ✗ Failed.");
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
            VerifyResult::HumanNeeded => (Outcome::HumanNeeded, None),
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
        println!("  → exec into next in_viewport step");
        let err = std::process::Command::new(&wf_bin)
            .args(["_run", task_name, &step_idx.to_string()])
            .exec();
        bail!("exec failed: {}", err);
    }

    let session = ctx.get("session").unwrap();

    project.viewport.open(task_name, ctx.get("repo_root").unwrap())?;

    println!("  → Sending to {}:{}", session, task_name);
    println!("  → Waiting for 'pawl done {}'", task_name);

    project.viewport.send(task_name, &run_cmd)?;

    Ok(())
}

// --- Verify helpers ---

#[derive(Debug, PartialEq)]
enum VerifyResult {
    Passed,
    HumanNeeded,
    Failed { feedback: String },
}

/// Run the verify command for a step, if any.
fn run_verify(project: &Project, task_name: &str, step: &Step, step_idx: usize) -> Result<VerifyResult> {
    match &step.verify {
        None => Ok(VerifyResult::Passed),
        Some(v) if v == "human" => Ok(VerifyResult::HumanNeeded),
        Some(cmd) => {
            let ctx = project.context_for(task_name, Some(step_idx));
            let expanded = ctx.expand(cmd);
            let env = ctx.to_env_vars();
            let result = run_command_with_env(&expanded, &env)?;

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
    fn test_decide_yield_verify_human() {
        assert_eq!(decide(Outcome::HumanNeeded, FailPolicy::Terminal), Verdict::Yield { reason: "verify_human" });
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
    fn test_decide_failure_human() {
        assert_eq!(
            decide(Outcome::Failure { feedback: "bad".into() }, FailPolicy::Human),
            Verdict::Yield { reason: "on_fail_human" }
        );
    }
}
