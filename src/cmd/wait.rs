use anyhow::Result;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::PawlError;
use crate::model::TaskStatus;

use super::common::Project;

/// Wait for task(s) to reach a specific status.
/// Single task: backward compatible. Multiple tasks: --any or all.
pub fn run(task_names: &[String], until: &str, timeout_secs: u64, interval_ms: u64, any: bool) -> Result<()> {
    if task_names.is_empty() {
        return Err(PawlError::Validation {
            message: "No task names provided".to_string(),
        }.into());
    }

    let targets = parse_statuses(until)?;
    let timeout = Duration::from_secs(timeout_secs);
    let interval = Duration::from_millis(interval_ms);
    let start = Instant::now();
    let project = Project::load()?;

    // Resolve all task names upfront
    let resolved: Vec<String> = task_names.iter()
        .map(|n| project.resolve_task_name(n))
        .collect::<Result<Vec<_>>>()?;

    // Check initial status for all tasks
    let mut reached: Vec<bool> = vec![false; resolved.len()];
    for (i, name) in resolved.iter().enumerate() {
        project.detect_viewport_loss(name)?;
        let status = current_status(&project, name)?;

        if targets.contains(&status) {
            reached[i] = true;
            if any {
                eprintln!(
                    "Task '{}' already at status '{}' after {:.1}s",
                    name, status, start.elapsed().as_secs_f64()
                );
                project.output_task_state(name)?;
                return Ok(());
            }
        } else if is_terminal_mismatch(status, &targets) && !any {
            return Err(PawlError::StateConflict {
                task: name.clone(),
                status: status.to_string(),
                message: format!("will not reach any of '{}'", until),
            }.into());
        }
    }

    // All already reached?
    if reached.iter().all(|r| *r) {
        for name in &resolved {
            let status = current_status(&project, name)?;
            eprintln!(
                "Task '{}' reached status '{}' after {:.1}s",
                name, status, start.elapsed().as_secs_f64()
            );
        }
        output_all(&project, &resolved)?;
        return Ok(());
    }

    // Poll loop
    loop {
        thread::sleep(interval);

        if start.elapsed() >= timeout {
            let statuses: Vec<String> = resolved.iter()
                .map(|n| {
                    let s = current_status(&project, n).unwrap_or(TaskStatus::Pending);
                    format!("{}={}", n, s)
                })
                .collect();
            return Err(PawlError::Timeout {
                message: format!(
                    "waiting for {} to reach '{}' (current: {})",
                    if resolved.len() == 1 { format!("task '{}'", resolved[0]) }
                    else { format!("{} tasks", resolved.len()) },
                    until,
                    statuses.join(", ")
                ),
            }.into());
        }

        for (i, name) in resolved.iter().enumerate() {
            if reached[i] { continue; }

            project.detect_viewport_loss(name)?;
            let status = current_status(&project, name)?;

            if targets.contains(&status) {
                reached[i] = true;
                if any {
                    eprintln!(
                        "Task '{}' reached status '{}' after {:.1}s",
                        name, status, start.elapsed().as_secs_f64()
                    );
                    project.output_task_state(name)?;
                    return Ok(());
                }
            } else if is_terminal_mismatch(status, &targets) && !any {
                return Err(PawlError::StateConflict {
                    task: name.clone(),
                    status: status.to_string(),
                    message: format!("will not reach any of '{}'", until),
                }.into());
            }
        }

        // All mode: check if all reached
        if !any && reached.iter().all(|r| *r) {
            for name in &resolved {
                let status = current_status(&project, name)?;
                eprintln!(
                    "Task '{}' reached status '{}' after {:.1}s",
                    name, status, start.elapsed().as_secs_f64()
                );
            }
            output_all(&project, &resolved)?;
            return Ok(());
        }
    }
}

fn current_status(project: &Project, task_name: &str) -> Result<TaskStatus> {
    Ok(project
        .replay_task(task_name)?
        .map(|s| s.status)
        .unwrap_or(TaskStatus::Pending))
}

fn output_all(project: &Project, tasks: &[String]) -> Result<()> {
    if tasks.len() == 1 {
        project.output_task_state(&tasks[0])?;
    } else {
        // Multi-task: output JSON array
        let mut results = Vec::new();
        for name in tasks {
            project.detect_viewport_loss(name)?;
            let state = project.replay_task(name)?;
            let events = project.read_events(name)?;
            let workflow_len = project.config.workflow.len();

            let (current_step, status, run_id, message) = if let Some(s) = &state {
                (s.current_step, s.status.to_string(), s.run_id.clone(), s.message.clone())
            } else {
                (0, "pending".to_string(), String::new(), None)
            };

            let (retry_count, last_feedback) = super::common::extract_step_context(&events, current_step);

            results.push(serde_json::json!({
                "name": name,
                "status": status,
                "run_id": run_id,
                "current_step": current_step,
                "step_name": project.step_name(current_step),
                "total_steps": workflow_len,
                "message": message,
                "retry_count": retry_count,
                "last_feedback": last_feedback,
            }));
        }
        println!("{}", serde_json::to_string(&results)?);
    }
    Ok(())
}

fn parse_statuses(s: &str) -> Result<Vec<TaskStatus>> {
    s.split(',')
        .map(|part| part.trim().parse::<TaskStatus>())
        .collect()
}

/// Terminal mismatch: current status cannot reach ANY of the targets
fn is_terminal_mismatch(current: TaskStatus, targets: &[TaskStatus]) -> bool {
    targets.iter().all(|t| current != *t && !current.can_reach(*t))
}
