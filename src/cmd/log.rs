use anyhow::{bail, Result};

use crate::model::StepLog;

use super::common::Project;

/// Show task logs
pub fn run(task_name: &str, step: Option<usize>, all: bool) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    // Verify task exists
    let _task = project.load_task(&task_name)?;

    // Check if task has been started
    let Some(state) = project.status.get(&task_name) else {
        bail!("Task '{}' has not been started yet.", task_name);
    };

    let log_file = project.log_file(&task_name);

    if !log_file.exists() {
        println!("No logs available for task '{}'.", task_name);
        println!("Logs are created when steps are executed.");
        return Ok(());
    }

    // Read all logs
    let logs = project.read_logs(&task_name)?;

    if logs.is_empty() {
        println!("No log entries found for task '{}'.", task_name);
        return Ok(());
    }

    if all {
        // Show all step logs
        for (i, log) in logs.iter().enumerate() {
            if i > 0 {
                println!("\n{}", "─".repeat(60));
            }
            print_log_entry(log, &project);
        }
    } else if let Some(step_num) = step {
        // Show specific step log (1-based)
        let step_idx = step_num.saturating_sub(1);

        // Find the log entry for this step
        let entry = logs.iter().find(|log| get_step_index(log) == step_idx);

        if let Some(entry) = entry {
            print_log_entry(entry, &project);
        } else {
            println!("No log entry found for step {}.", step_num);
            println!("The step may not have been executed yet.");
        }
    } else {
        // Show the most recent step log (current or last executed)
        let target_step = if state.current_step > 0 {
            state.current_step - 1
        } else {
            0
        };

        // Find the log entry for this step, or show the last one
        let entry = logs
            .iter()
            .rev()
            .find(|log| get_step_index(log) == target_step)
            .or_else(|| logs.last());

        if let Some(entry) = entry {
            print_log_entry(entry, &project);
        } else {
            println!("No log entries found.");
        }
    }

    Ok(())
}

/// Get the step index from a log entry
fn get_step_index(log: &StepLog) -> usize {
    match log {
        StepLog::Command { step, .. } => *step,
        StepLog::InWindow { step, .. } => *step,
        StepLog::Checkpoint { step } => *step,
    }
}

/// Print a formatted log entry
fn print_log_entry(log: &StepLog, project: &Project) {
    match log {
        StepLog::Command {
            step,
            exit_code,
            duration,
            stdout,
            stderr,
        } => {
            let step_name = project
                .config
                .workflow
                .get(*step)
                .map(|s| s.name.as_str())
                .unwrap_or("Unknown");

            println!("=== Step {}: {} (command) ===", step + 1, step_name);
            println!("Exit code: {}", exit_code);
            println!("Duration: {:.1}s", duration);

            if !stdout.is_empty() {
                println!("\n[stdout]");
                // Limit output to avoid overwhelming the terminal
                let lines: Vec<&str> = stdout.lines().collect();
                if lines.len() > 50 {
                    for line in lines.iter().take(25) {
                        println!("{}", line);
                    }
                    println!("... ({} lines omitted) ...", lines.len() - 50);
                    for line in lines.iter().skip(lines.len() - 25) {
                        println!("{}", line);
                    }
                } else {
                    print!("{}", stdout);
                    if !stdout.ends_with('\n') {
                        println!();
                    }
                }
            }

            if !stderr.is_empty() {
                println!("\n[stderr]");
                print!("{}", stderr);
                if !stderr.ends_with('\n') {
                    println!();
                }
            }
        }

        StepLog::InWindow {
            step,
            session_id,
            transcript,
            status,
        } => {
            let step_name = project
                .config
                .workflow
                .get(*step)
                .map(|s| s.name.as_str())
                .unwrap_or("Unknown");

            println!("=== Step {}: {} (in_window) ===", step + 1, step_name);
            println!("Status: {}", status);

            if let Some(sid) = session_id {
                println!("Session ID: {}", sid);
            }

            if let Some(path) = transcript {
                println!("Transcript: {}", path);
            }
        }

        StepLog::Checkpoint { step } => {
            let step_name = project
                .config
                .workflow
                .get(*step)
                .map(|s| s.name.as_str())
                .unwrap_or("Unknown");

            println!("=== Step {}: {} (checkpoint) ===", step + 1, step_name);
            println!("Checkpoint reached.");
        }
    }
}
