use anyhow::{bail, Result};
use fs2::FileExt;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use crate::model::config::Step;
use crate::model::event::{event_timestamp, replay, Event};
use crate::model::state::TaskState;
use crate::model::TaskStatus;
use crate::util::tmux;

use super::common::Project;

/// Wait for task to reach a specific status (supports comma-separated multi-status)
pub fn run(task_name: &str, until: &str, timeout_secs: u64, interval_ms: u64) -> Result<()> {
    let targets = parse_statuses(until)?;

    let timeout = Duration::from_secs(timeout_secs);
    let interval = Duration::from_millis(interval_ms);
    let start = Instant::now();

    // First iteration: load full project to resolve task name and get paths
    let project = Project::load()?;
    let resolved_name = project.resolve_task_name(task_name)?;
    let wf_dir = project.wf_dir.clone();
    let workflow_len = project.config.workflow.len();
    let workflow = project.config.workflow.clone();
    let session_name = project.session_name();

    // Check initial status
    let current_status = project
        .replay_task(&resolved_name)?
        .map(|s| s.status)
        .unwrap_or(TaskStatus::Pending);

    if targets.contains(&current_status) {
        println!(
            "Task '{}' reached status '{:?}' after {:.1}s",
            resolved_name,
            current_status,
            start.elapsed().as_secs_f64()
        );
        return Ok(());
    }

    if is_terminal_mismatch_multi(current_status, &targets) {
        bail!(
            "Task '{}' is in terminal state '{:?}', will not reach any of '{}'",
            resolved_name,
            current_status,
            until
        );
    }

    drop(project);

    // Subsequent iterations: only read JSONL + replay (skip Config and git calls)
    poll_status(&resolved_name, &targets, until, &wf_dir, workflow_len, &workflow, &session_name, timeout, interval, start)
}

/// Poll status by reading JSONL and replaying
fn poll_status(
    task_name: &str,
    targets: &[TaskStatus],
    until: &str,
    wf_dir: &PathBuf,
    workflow_len: usize,
    workflow: &[Step],
    session_name: &str,
    timeout: Duration,
    interval: Duration,
    start: Instant,
) -> Result<()> {
    let log_file = wf_dir.join("logs").join(format!("{}.jsonl", task_name));

    loop {
        thread::sleep(interval);

        if start.elapsed() >= timeout {
            let current_status = replay_state_from_file(&log_file, workflow_len)?
                .map(|s| s.status)
                .unwrap_or(TaskStatus::Pending);
            bail!(
                "Timeout waiting for task '{}' to reach status '{}' (current: {:?})",
                task_name,
                until,
                current_status
            );
        }

        let state = replay_state_from_file(&log_file, workflow_len)?;

        // Health check: if Running on in_window step but window is gone, emit WindowLost
        if let Some(ref s) = state {
            if s.status == TaskStatus::Running {
                let step_idx = s.current_step;
                if step_idx < workflow.len()
                    && workflow[step_idx].in_window
                    && !tmux::window_exists(session_name, task_name)
                {
                    append_window_lost(&log_file, step_idx)?;
                    continue; // next iteration will replay to Failed
                }
            }
        }

        let current_status = state.map(|s| s.status).unwrap_or(TaskStatus::Pending);

        if targets.contains(&current_status) {
            println!(
                "Task '{}' reached status '{:?}' after {:.1}s",
                task_name,
                current_status,
                start.elapsed().as_secs_f64()
            );
            return Ok(());
        }

        if is_terminal_mismatch_multi(current_status, targets) {
            bail!(
                "Task '{}' is in terminal state '{:?}', will not reach any of '{}'",
                task_name,
                current_status,
                until
            );
        }
    }
}

/// Read events from JSONL file and replay to get current TaskState
fn replay_state_from_file(log_file: &PathBuf, workflow_len: usize) -> Result<Option<TaskState>> {
    if !log_file.exists() {
        return Ok(None);
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

    Ok(replay(&events, workflow_len))
}

/// Append a WindowLost event directly to the JSONL file (with file lock).
fn append_window_lost(log_file: &PathBuf, step_idx: usize) -> Result<()> {
    let event = Event::WindowLost {
        ts: event_timestamp(),
        step: step_idx,
    };
    let json = serde_json::to_string(&event)?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)?;

    file.lock_exclusive()?;
    writeln!(file, "{}", json)?;
    file.unlock()?;

    Ok(())
}

fn parse_statuses(s: &str) -> Result<Vec<TaskStatus>> {
    s.split(',')
        .map(|part| {
            match part.trim().to_lowercase().as_str() {
                "pending" => Ok(TaskStatus::Pending),
                "running" => Ok(TaskStatus::Running),
                "waiting" => Ok(TaskStatus::Waiting),
                "completed" => Ok(TaskStatus::Completed),
                "failed" => Ok(TaskStatus::Failed),
                "stopped" => Ok(TaskStatus::Stopped),
                _ => bail!(
                    "Invalid status '{}'. Valid values: pending, running, waiting, completed, failed, stopped",
                    part.trim()
                ),
            }
        })
        .collect()
}

/// Terminal mismatch: current status cannot reach ANY of the targets
fn is_terminal_mismatch_multi(current: TaskStatus, targets: &[TaskStatus]) -> bool {
    targets.iter().all(|t| current != *t && !current.can_reach(*t))
}
