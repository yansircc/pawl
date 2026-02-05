use anyhow::{bail, Result};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use crate::model::{StatusStore, TaskStatus};

use super::common::Project;

/// Wait for task to reach a specific status
pub fn run(task_name: &str, until: &str, timeout_secs: u64, interval_ms: u64) -> Result<()> {
    // Parse target status
    let target_status = parse_status(until)?;

    let timeout = Duration::from_secs(timeout_secs);
    let interval = Duration::from_millis(interval_ms);
    let start = Instant::now();

    // First iteration: load full project to resolve task name and get wf_dir
    let project = Project::load()?;
    let resolved_name = project.resolve_task_name(task_name)?;
    let wf_dir = project.wf_dir.clone();

    // Check initial status
    let current_status = project
        .status
        .get(&resolved_name)
        .map(|s| s.status)
        .unwrap_or(TaskStatus::Pending);

    if current_status == target_status {
        println!(
            "Task '{}' reached status '{}' after {:.1}s",
            resolved_name,
            until,
            start.elapsed().as_secs_f64()
        );
        return Ok(());
    }

    if is_terminal_mismatch(current_status, target_status) {
        bail!(
            "Task '{}' is in terminal state '{:?}', will not reach '{}'",
            resolved_name,
            current_status,
            until
        );
    }

    // Drop the full project, keep only what we need
    drop(project);

    // Subsequent iterations: only load StatusStore (skip Config and git rev-parse)
    poll_status(&resolved_name, target_status, until, &wf_dir, timeout, interval, start)
}

/// Poll status without reloading full project
fn poll_status(
    task_name: &str,
    target_status: TaskStatus,
    until: &str,
    wf_dir: &PathBuf,
    timeout: Duration,
    interval: Duration,
    start: Instant,
) -> Result<()> {
    loop {
        thread::sleep(interval);

        // Check timeout first
        if start.elapsed() >= timeout {
            // Load final status for error message
            let status = StatusStore::load(wf_dir)?;
            let current_status = status
                .get(task_name)
                .map(|s| s.status)
                .unwrap_or(TaskStatus::Pending);
            bail!(
                "Timeout waiting for task '{}' to reach status '{}' (current: {:?})",
                task_name,
                until,
                current_status
            );
        }

        // Load only status (fast path - no Config parsing, no git calls)
        let status = StatusStore::load(wf_dir)?;
        let current_status = status
            .get(task_name)
            .map(|s| s.status)
            .unwrap_or(TaskStatus::Pending);

        if current_status == target_status {
            println!(
                "Task '{}' reached status '{}' after {:.1}s",
                task_name,
                until,
                start.elapsed().as_secs_f64()
            );
            return Ok(());
        }

        // Check for terminal states that won't change to target
        if is_terminal_mismatch(current_status, target_status) {
            bail!(
                "Task '{}' is in terminal state '{:?}', will not reach '{}'",
                task_name,
                current_status,
                until
            );
        }
    }
}

fn parse_status(s: &str) -> Result<TaskStatus> {
    match s.to_lowercase().as_str() {
        "pending" => Ok(TaskStatus::Pending),
        "running" => Ok(TaskStatus::Running),
        "waiting" => Ok(TaskStatus::Waiting),
        "completed" => Ok(TaskStatus::Completed),
        "failed" => Ok(TaskStatus::Failed),
        "stopped" => Ok(TaskStatus::Stopped),
        _ => bail!(
            "Invalid status '{}'. Valid values: pending, running, waiting, completed, failed, stopped",
            s
        ),
    }
}

/// Check if current status is terminal and can't reach target
fn is_terminal_mismatch(current: TaskStatus, target: TaskStatus) -> bool {
    // If already at target, not a mismatch
    if current == target {
        return false;
    }

    // Completed/Failed/Stopped are terminal states
    // They can only be changed by explicit user action (reset, retry)
    match current {
        TaskStatus::Completed => {
            // Completed can't naturally transition to anything else
            true
        }
        TaskStatus::Failed | TaskStatus::Stopped => {
            // Failed/Stopped can't naturally reach running/waiting/completed
            matches!(
                target,
                TaskStatus::Running | TaskStatus::Waiting | TaskStatus::Completed
            )
        }
        _ => false,
    }
}
