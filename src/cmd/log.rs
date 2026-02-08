use anyhow::{bail, Result};
use std::io::{BufRead, BufReader};

use crate::model::Event;

use super::common::Project;

/// Show task logs (JSONL output)
pub fn run(task_name: &str, step: Option<usize>, all: bool, all_runs: bool) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    // Verify task exists
    let _task = project.load_task(&task_name)?;

    // Check if task has been started
    let state = project.replay_task(&task_name)?;
    if state.is_none() {
        bail!("Task '{}' has not been started yet.", task_name);
    }

    let log_file = project.log_file(&task_name);

    if !log_file.exists() {
        return Ok(());
    }

    // Read raw lines for JSONL output
    let file = std::fs::File::open(&log_file)?;
    let reader = BufReader::new(file);

    let lines: Vec<String> = reader
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .collect();

    // Filter to current run unless --all-runs
    let lines = if all_runs {
        lines
    } else {
        current_run_lines(lines)
    };

    if all || step.is_some() {
        for line in &lines {
            if let Some(idx) = step {
                let event: Event = serde_json::from_str(line)?;
                if event.step_index() != Some(idx) {
                    continue;
                }
            }
            println!("{}", line);
        }
    } else {
        // Default: last event only
        if let Some(line) = lines.last() {
            println!("{}", line);
        }
    }

    Ok(())
}

/// Filter JSONL lines to only the current run (after the last task_reset line).
fn current_run_lines(lines: Vec<String>) -> Vec<String> {
    let last_reset_pos = lines
        .iter()
        .rposition(|l| l.contains("\"type\":\"task_reset\""));

    match last_reset_pos {
        Some(pos) => lines.into_iter().skip(pos + 1).collect(),
        None => lines,
    }
}
