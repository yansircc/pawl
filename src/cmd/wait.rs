use anyhow::{bail, Result};
use std::thread;
use std::time::{Duration, Instant};

use crate::model::TaskStatus;

use super::common::Project;

/// Wait for task to reach a specific status
pub fn run(task_name: &str, until: &str, timeout_secs: u64, interval_ms: u64) -> Result<()> {
    // Parse target status
    let target_status = parse_status(until)?;

    let timeout = Duration::from_secs(timeout_secs);
    let interval = Duration::from_millis(interval_ms);
    let start = Instant::now();

    loop {
        // Load fresh project state each iteration
        let project = Project::load()?;
        let task_name = project.resolve_task_name(task_name)?;

        let current_status = project
            .status
            .get(&task_name)
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

        // Check timeout
        if start.elapsed() >= timeout {
            bail!(
                "Timeout waiting for task '{}' to reach status '{}' (current: {:?})",
                task_name,
                until,
                current_status
            );
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

        thread::sleep(interval);
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
