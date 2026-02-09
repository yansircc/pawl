use anyhow::Result;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::PawlError;
use crate::model::TaskStatus;

use super::common::Project;

/// Wait for task to reach a specific status (supports comma-separated multi-status)
pub fn run(task_name: &str, until: &str, timeout_secs: u64, interval_ms: u64) -> Result<()> {
    let targets = parse_statuses(until)?;
    let timeout = Duration::from_secs(timeout_secs);
    let interval = Duration::from_millis(interval_ms);
    let start = Instant::now();

    let project = Project::load()?;
    let resolved_name = project.resolve_task_name(task_name)?;

    // Check initial status
    project.detect_viewport_loss(&resolved_name)?;
    let current_status = project
        .replay_task(&resolved_name)?
        .map(|s| s.status)
        .unwrap_or(TaskStatus::Pending);

    if targets.contains(&current_status) {
        eprintln!(
            "Task '{}' reached status '{}' after {:.1}s",
            resolved_name,
            current_status,
            start.elapsed().as_secs_f64()
        );
        project.output_task_state(&resolved_name)?;
        return Ok(());
    }

    if is_terminal_mismatch_multi(current_status, &targets) {
        return Err(PawlError::StateConflict {
            task: resolved_name.clone(),
            status: current_status.to_string(),
            message: format!("will not reach any of '{}'", until),
        }.into());
    }

    poll_status(&project, &resolved_name, &targets, until, timeout, interval, start)
}

/// Poll status using Project API
fn poll_status(
    project: &Project,
    task_name: &str,
    targets: &[TaskStatus],
    until: &str,
    timeout: Duration,
    interval: Duration,
    start: Instant,
) -> Result<()> {
    loop {
        thread::sleep(interval);

        if start.elapsed() >= timeout {
            let current_status = project
                .replay_task(task_name)?
                .map(|s| s.status)
                .unwrap_or(TaskStatus::Pending);
            return Err(PawlError::Timeout {
                message: format!("waiting for task '{}' to reach '{}' (current: {})", task_name, until, current_status),
            }.into());
        }

        // Health check: unified through Project API
        project.detect_viewport_loss(task_name)?;

        let current_status = project
            .replay_task(task_name)?
            .map(|s| s.status)
            .unwrap_or(TaskStatus::Pending);

        if targets.contains(&current_status) {
            eprintln!(
                "Task '{}' reached status '{}' after {:.1}s",
                task_name,
                current_status,
                start.elapsed().as_secs_f64()
            );
            project.output_task_state(task_name)?;
            return Ok(());
        }

        if is_terminal_mismatch_multi(current_status, targets) {
            return Err(PawlError::StateConflict {
                task: task_name.to_string(),
                status: current_status.to_string(),
                message: format!("will not reach any of '{}'", until),
            }.into());
        }
    }
}

fn parse_statuses(s: &str) -> Result<Vec<TaskStatus>> {
    s.split(',')
        .map(|part| part.trim().parse::<TaskStatus>())
        .collect()
}

/// Terminal mismatch: current status cannot reach ANY of the targets
fn is_terminal_mismatch_multi(current: TaskStatus, targets: &[TaskStatus]) -> bool {
    targets.iter().all(|t| current != *t && !current.can_reach(*t))
}
