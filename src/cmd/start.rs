use anyhow::{bail, Result};

use crate::model::{Step, StepStatus, TaskState, TaskStatus};
use crate::util::shell::{run_command_with_env, spawn_background};
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
            fire_hook(project, "task.completed", task_name);
            return Ok(());
        }

        // Clone the step to avoid borrow conflicts
        let step = project.config.workflow[step_idx].clone();
        let worktree_dir = project.config.worktree_dir.clone();
        let repo_root = project.repo_root.clone();

        let ctx = Context::new(
            task_name,
            &session,
            &repo_root,
            &worktree_dir,
            &step.name,
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
            fire_hook(project, "checkpoint", task_name);
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
    {
        let state = project.status.get_mut(task_name).unwrap();
        state.status = TaskStatus::Running;
        state.touch();
    }
    project.save_status()?;

    // Run command with environment variables
    let env = ctx.to_env_vars();
    let result = run_command_with_env(command, &env)?;

    let step_idx = {
        let state = project.status.get(task_name).unwrap();
        state.current_step
    };

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
        fire_hook(project, "step.success", task_name);
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
        fire_hook(project, "step.failed", task_name);
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

    // Wrap command with on-exit handler
    // Format: command; wf _on-exit task_name $?
    let wrapped = format!("{}; wf _on-exit {} $?", command, task_name);

    // Send to tmux window
    let session = &ctx.session;
    let window = &ctx.window;

    // Check if window exists
    if !tmux::window_exists(session, window) {
        // Window doesn't exist - the workflow should have created it
        // Try to create it in the worktree directory
        println!("  Creating window {}:{}...", session, window);
        tmux::create_window(session, window, Some(&ctx.worktree))?;
    }

    println!("  → Sending to {}:{}", session, window);
    println!("  → Waiting for 'wf done {}' or 'wf fail {}'", task_name, task_name);

    tmux::send_keys(session, window, &wrapped)?;

    Ok(())
}

/// Fire a hook (fire-and-forget)
fn fire_hook(project: &Project, event: &str, task_name: &str) {
    if let Some(cmd) = project.config.hooks.get(event) {
        let ctx = Context::new(
            task_name,
            &project.session_name(),
            &project.repo_root,
            &project.config.worktree_dir,
            event,
        );
        let expanded = ctx.expand(cmd);
        if let Err(e) = spawn_background(&expanded) {
            eprintln!("Warning: hook '{}' failed: {}", event, e);
        }
    }
}
