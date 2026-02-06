use anyhow::{bail, Result};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use crate::model::event::{replay, Event};
use crate::model::TaskStatus;

use super::common::Project;

/// Wait for task to reach a specific status
pub fn run(task_name: &str, until: &str, timeout_secs: u64, interval_ms: u64) -> Result<()> {
    let target_status = parse_status(until)?;

    let timeout = Duration::from_secs(timeout_secs);
    let interval = Duration::from_millis(interval_ms);
    let start = Instant::now();

    // First iteration: load full project to resolve task name and get paths
    let project = Project::load()?;
    let resolved_name = project.resolve_task_name(task_name)?;
    let wf_dir = project.wf_dir.clone();
    let workflow_len = project.config.workflow.len();

    // Check initial status
    let current_status = project
        .replay_task(&resolved_name)?
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

    drop(project);

    // Subsequent iterations: only read JSONL + replay (skip Config and git calls)
    poll_status(&resolved_name, target_status, until, &wf_dir, workflow_len, timeout, interval, start)
}

/// Poll status by reading JSONL and replaying
fn poll_status(
    task_name: &str,
    target_status: TaskStatus,
    until: &str,
    wf_dir: &PathBuf,
    workflow_len: usize,
    timeout: Duration,
    interval: Duration,
    start: Instant,
) -> Result<()> {
    let log_file = wf_dir.join("logs").join(format!("{}.jsonl", task_name));

    loop {
        thread::sleep(interval);

        if start.elapsed() >= timeout {
            let current_status = replay_from_file(&log_file, workflow_len)?;
            bail!(
                "Timeout waiting for task '{}' to reach status '{}' (current: {:?})",
                task_name,
                until,
                current_status
            );
        }

        let current_status = replay_from_file(&log_file, workflow_len)?;

        if current_status == target_status {
            println!(
                "Task '{}' reached status '{}' after {:.1}s",
                task_name,
                until,
                start.elapsed().as_secs_f64()
            );
            return Ok(());
        }

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

/// Read events from JSONL file and replay to get current status
fn replay_from_file(log_file: &PathBuf, workflow_len: usize) -> Result<TaskStatus> {
    if !log_file.exists() {
        return Ok(TaskStatus::Pending);
    }

    let file = std::fs::File::open(log_file)?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event: Event = serde_json::from_str(&line)?;
        events.push(event);
    }

    Ok(replay(&events, workflow_len)
        .map(|s| s.status)
        .unwrap_or(TaskStatus::Pending))
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

fn is_terminal_mismatch(current: TaskStatus, target: TaskStatus) -> bool {
    current != target && !current.can_reach(target)
}
