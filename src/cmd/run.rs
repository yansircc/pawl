use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{bail, Result};

use crate::model::TaskStatus;
use super::common::Project;
use super::start::{resume_workflow, settle_step, StepRecord};

/// Internal: run a command in viewport as the parent process.
/// Replaces the old runner-script + EXIT-trap + `pawl _on-exit` chain.
pub fn run_in_viewport(task_name: &str, step_idx: usize) -> Result<()> {
    // 1. Ignore SIGHUP so we survive viewport close
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }

    // 2. Mark that we're running inside a viewport (for consecutive in_viewport steps)
    unsafe {
        std::env::set_var("PAWL_IN_VIEWPORT", task_name);
    }

    // 3. Load project, verify state
    let project = Project::load()?;
    let state = project.replay_task(task_name)?;

    let Some(state) = state else {
        return Ok(());
    };
    if state.status != TaskStatus::Running || state.current_step != step_idx {
        return Ok(());
    }

    if step_idx >= project.config.workflow.len() {
        return Ok(());
    }

    let step = &project.config.workflow[step_idx];
    let command = match &step.run {
        Some(cmd) => cmd.clone(),
        None => bail!("Step {} has no run command", step_idx),
    };

    // 4. Build context, expand command, prepare env vars
    let mut ctx = project.context_for(task_name, Some(step_idx), &state.run_id);
    let events = project.read_events(task_name)?;
    let (retry_count, last_feedback) = super::common::extract_step_context(&events, step_idx);
    ctx = ctx.var("retry_count", retry_count.to_string());
    if let Some(fb) = &last_feedback {
        ctx = ctx.var("last_verify_output", fb);
    }

    let expanded = ctx.expand(&command);
    let env = ctx.to_env_vars();

    // Use project_root as working directory; user vars can override via command `cd ${worktree} && ...`
    let work_dir = &project.project_root;

    // 5. Fork child process (bash -c), inherit stdio for viewport interactivity
    let start_time = Instant::now();

    let mut child = unsafe {
        Command::new("bash")
            .arg("-c")
            .arg(&expanded)
            .current_dir(work_dir)
            .envs(&env)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .pre_exec(|| {
                // Restore default SIGHUP handling in child
                libc::signal(libc::SIGHUP, libc::SIG_DFL);
                Ok(())
            })
            .spawn()?
    };

    // 6. Wait for child (OS-guaranteed delivery)
    let status = child.wait()?;
    let exit_code = status.code().unwrap_or(128);
    let elapsed = start_time.elapsed().as_secs_f64();

    // 7. Redirect stdout/stderr to /dev/null (pty may be closed after viewport close)
    redirect_to_devnull();

    // 8. Re-check state (pawl done may have already handled this step)
    let project = Project::load()?;
    let state = project.replay_task(task_name)?;

    let Some(state) = state else {
        return Ok(());
    };
    if state.status != TaskStatus::Running || state.current_step != step_idx {
        return Ok(());
    }

    // 9. Use unified pipeline
    let step = project.config.workflow[step_idx].clone();
    let record = StepRecord {
        exit_code,
        duration: Some(elapsed),
        stdout: None,
        stderr: None,
    };

    match settle_step(&project, task_name, step_idx, &step, record)? {
        true => {
            // Pipeline says continue — check if next step is also in_viewport
            // If so, execute() will detect PAWL_IN_VIEWPORT and exec into next pawl _run
            resume_workflow(&project, task_name)?;
        }
        false => {}
    }

    Ok(())
}

/// Redirect stdout and stderr to /dev/null.
/// After viewport close, the pty is gone and any write to stdout/stderr
/// would cause a broken pipe panic in Rust's println! macro.
fn redirect_to_devnull() {
    unsafe {
        let devnull = libc::open(b"/dev/null\0".as_ptr() as *const _, libc::O_WRONLY);
        if devnull >= 0 {
            libc::dup2(devnull, libc::STDOUT_FILENO);
            libc::dup2(devnull, libc::STDERR_FILENO);
            libc::close(devnull);
        }
    }
}
