use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use anyhow::{bail, Result};

use crate::model::TaskStatus;
use crate::util::variable::Context;

use super::common::Project;
use super::start::{continue_execution, handle_step_completion, RunOutput};

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

    let expanded = ctx.expand(&command);
    let env = ctx.to_env_vars();

    let work_dir = if std::path::Path::new(&ctx.worktree).exists() {
        &ctx.worktree
    } else {
        &ctx.repo_root
    };

    // 5. Fork child process (bash -c), inherit stdio for viewport interactivity
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
    let run_output = RunOutput {
        duration: None,
        stdout: None,
        stderr: None,
    };

    match handle_step_completion(&project, task_name, step_idx, exit_code, &step, run_output)? {
        true => {
            // Pipeline says continue — check if next step is also in_viewport
            // If so, execute() will detect PAWL_IN_VIEWPORT and exec into next pawl _run
            continue_execution(&project, task_name)?;
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
