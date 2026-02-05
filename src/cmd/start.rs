use anyhow::{bail, Result};
use chrono::Utc;
use std::fs;
use std::io::Write;

use crate::model::{StepStatus, TaskState, TaskStatus};
use crate::util::shell::{run_command_with_env, CommandResult};
use crate::util::tmux;
use crate::util::variable::Context;

use super::common::Project;

pub fn run(task_name: &str) -> Result<()> {
    let mut project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;
    let task_def = project.load_task(&task_name)?;

    // Check if task is already running
    if let Some(state) = project.status.get(&task_name) {
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

    // Initialize task state
    let state = TaskState::started();
    project.status.set(task_name.clone(), state);
    project.save_status()?;

    println!("Starting task: {}", task_name);

    // Fire task.started hook
    project.fire_hook("task.started", &task_name);

    // Execute the workflow
    execute(&mut project, &task_name)?;

    Ok(())
}

/// Continue execution from current step (called by wf next, wf done, etc.)
pub fn continue_execution(project: &mut Project, task_name: &str) -> Result<()> {
    execute(project, task_name)
}

/// Execute workflow steps starting from current_step
fn execute(project: &mut Project, task_name: &str) -> Result<()> {
    loop {
        // Get current state
        let step_idx = {
            let state = project.status.get(task_name).expect("Task state missing");
            state.current_step
        };

        // Clone needed config data to avoid borrow conflicts
        let workflow_len = project.config.workflow.len();
        let session = project.session_name();

        // Check if we've completed all steps
        if step_idx >= workflow_len {
            let state = project.status.get_mut(task_name).unwrap();
            state.status = TaskStatus::Completed;
            state.touch();
            project.save_status()?;
            println!("Task '{}' completed!", task_name);
            project.fire_hook("task.completed", task_name);
            return Ok(());
        }

        // Clone the step to avoid borrow conflicts
        let step = project.config.workflow[step_idx].clone();
        let worktree_dir = project.config.worktree_dir.clone();
        let repo_root = project.repo_root.clone();

        // Build context with log information
        let log_dir = project.log_dir(task_name);
        let log_path = project.log_path(task_name, step_idx, &step.name);
        let prev_log = project.prev_log_path(task_name, step_idx);
        let prev_log_str = prev_log.as_ref().map(|p| p.to_string_lossy().to_string());

        let ctx = Context::new_full(
            task_name,
            &session,
            &repo_root,
            &worktree_dir,
            &step.name,
            step_idx,
            &log_dir.to_string_lossy(),
            &log_path.to_string_lossy(),
            prev_log_str.as_deref(),
        );

        println!(
            "[{}/{}] {}",
            step_idx + 1,
            workflow_len,
            step.name
        );

        // Handle different step types
        if step.is_checkpoint() {
            // Checkpoint: pause and wait for wf next
            let state = project.status.get_mut(task_name).unwrap();
            state.status = TaskStatus::Waiting;
            state.touch();
            project.save_status()?;
            println!("  → Checkpoint. Use 'wf next {}' to continue.", task_name);
            project.fire_hook("checkpoint", task_name);
            return Ok(());
        }

        let command = step.run.as_ref().unwrap();
        let expanded = ctx.expand(command);

        if step.in_window {
            // in_window step: send to tmux window, don't wait
            execute_in_window(project, task_name, &ctx, &expanded)?;
            return Ok(());
        } else {
            // Normal step: execute synchronously
            let success = execute_step(project, task_name, &ctx, &expanded)?;
            if !success {
                return Ok(());
            }
            // Continue to next step
        }
    }
}

/// Execute a normal (synchronous) step
fn execute_step(
    project: &mut Project,
    task_name: &str,
    ctx: &Context,
    command: &str,
) -> Result<bool> {
    let step_idx = {
        let state = project.status.get(task_name).unwrap();
        state.current_step
    };

    let step_name = project.config.workflow[step_idx].name.clone();

    {
        let state = project.status.get_mut(task_name).unwrap();
        state.status = TaskStatus::Running;
        state.touch();
    }
    project.save_status()?;

    // Record start time
    let start_time = Utc::now();

    // Run command with environment variables
    let env = ctx.to_env_vars();
    let result = run_command_with_env(command, &env)?;

    // Record end time and calculate duration
    let end_time = Utc::now();
    let duration = end_time.signed_duration_since(start_time);

    // Write step log (best-effort)
    let status_str = if result.success { "success" } else { "failed" };
    write_step_log(
        project,
        task_name,
        step_idx,
        &step_name,
        command,
        &start_time,
        duration.num_milliseconds() as f64 / 1000.0,
        &result,
        status_str,
    );

    if result.success {
        // Success: mark step and advance
        {
            let state = project.status.get_mut(task_name).unwrap();
            state.mark_step(step_idx, StepStatus::Success);
            state.current_step += 1;
            state.message = None;
        }
        project.save_status()?;
        println!("  ✓ Done");
        project.fire_hook("step.success", task_name);
        Ok(true)
    } else {
        // Failure: mark step and stop
        {
            let state = project.status.get_mut(task_name).unwrap();
            state.mark_step(step_idx, StepStatus::Failed);
            state.status = TaskStatus::Failed;
            state.message = Some(format!("Exit code: {}", result.exit_code));
        }
        project.save_status()?;
        println!("  ✗ Failed (exit code {})", result.exit_code);
        if !result.stderr.is_empty() {
            // Print first few lines of stderr
            for line in result.stderr.lines().take(5) {
                println!("    {}", line);
            }
        }
        project.fire_hook("step.failed", task_name);
        project.fire_hook("task.failed", task_name);
        Ok(false)
    }
}

/// Execute an in_window step (send to tmux)
fn execute_in_window(
    project: &mut Project,
    task_name: &str,
    ctx: &Context,
    command: &str,
) -> Result<()> {
    {
        let state = project.status.get_mut(task_name).unwrap();
        state.status = TaskStatus::Running;
        state.touch();
    }
    project.save_status()?;

    // Send to tmux window
    let session = &ctx.session;
    let window = &ctx.window;

    // Check if session exists, create if not
    if !tmux::session_exists(session) {
        println!("  Creating session {}...", session);
        tmux::create_session(session, Some(&ctx.repo_root))?;
    }

    // Check if window exists
    if !tmux::window_exists(session, window) {
        // Window doesn't exist - create it in the repo root
        println!("  Creating window {}:{}...", session, window);
        tmux::create_window(session, window, Some(&ctx.repo_root))?;
    }

    // Determine working directory: use worktree if it exists, otherwise repo_root
    let work_dir = if std::path::Path::new(&ctx.worktree).exists() {
        &ctx.worktree
    } else {
        &ctx.repo_root
    };

    // Wrap command with cd and on-exit handler
    // Note: _on-exit must run in repo_root where .wf directory exists
    // Format: cd <work_dir> && command; cd <repo_root> && wf _on-exit task_name $?
    let wrapped = format!(
        "cd '{}' && {}; __wf_exit=$?; cd '{}' && wf _on-exit {} $__wf_exit",
        work_dir, command, ctx.repo_root, task_name
    );

    println!("  → Sending to {}:{}", session, window);
    println!("  → Waiting for 'wf done {}' or 'wf fail {}'", task_name, task_name);

    tmux::send_keys(session, window, &wrapped)?;

    Ok(())
}

/// Write step log file (best-effort, errors are silently ignored)
fn write_step_log(
    project: &Project,
    task_name: &str,
    step_idx: usize,
    step_name: &str,
    command: &str,
    start_time: &chrono::DateTime<Utc>,
    duration_secs: f64,
    result: &CommandResult,
    status: &str,
) {
    let log_dir = project.log_dir(task_name);
    let log_path = project.log_path(task_name, step_idx, step_name);

    // Create log directory if it doesn't exist
    if fs::create_dir_all(&log_dir).is_err() {
        return;
    }

    let content = format!(
        "=== Step {}: {} ===\n\
         Command: {}\n\
         Started: {}\n\
         \n\
         {}\
         \n\
         Exit code: {}\n\
         Duration: {:.1}s\n\
         Status: {}\n",
        step_idx + 1,
        step_name,
        command,
        start_time.to_rfc3339(),
        format_output(&result.stdout, &result.stderr),
        result.exit_code,
        duration_secs,
        status,
    );

    // Best-effort write
    if let Ok(mut file) = fs::File::create(&log_path) {
        let _ = file.write_all(content.as_bytes());
    }
}

/// Format stdout/stderr for log output
fn format_output(stdout: &str, stderr: &str) -> String {
    let mut output = String::new();

    if !stdout.is_empty() {
        output.push_str("[stdout]\n");
        output.push_str(stdout);
        if !stdout.ends_with('\n') {
            output.push('\n');
        }
    }

    if !stderr.is_empty() {
        output.push_str("[stderr]\n");
        output.push_str(stderr);
        if !stderr.ends_with('\n') {
            output.push('\n');
        }
    }

    if output.is_empty() {
        output.push_str("[no output]\n");
    }

    output
}

