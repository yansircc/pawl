use anyhow::{bail, Result};
use std::fs;

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

    let log_dir = project.log_dir(&task_name);

    if !log_dir.exists() {
        println!("No logs available for task '{}'.", task_name);
        println!("Logs are created when steps are executed.");
        return Ok(());
    }

    if all {
        // Show all step logs in order
        show_all_logs(&project, &task_name)?;
    } else if let Some(step_num) = step {
        // Show specific step log (1-based)
        show_step_log(&project, &task_name, step_num)?;
    } else {
        // Show the most recent step log (current or last executed)
        let step_idx = if state.current_step > 0 {
            state.current_step - 1
        } else {
            0
        };
        show_step_log(&project, &task_name, step_idx + 1)?;
    }

    Ok(())
}

/// Show log for a specific step (1-based index)
fn show_step_log(project: &Project, task_name: &str, step_num: usize) -> Result<()> {
    let step_idx = step_num.saturating_sub(1);

    if step_idx >= project.config.workflow.len() {
        bail!(
            "Step {} does not exist. Task has {} steps.",
            step_num,
            project.config.workflow.len()
        );
    }

    let step_name = &project.config.workflow[step_idx].name;
    let log_path = project.log_path(task_name, step_idx, step_name);

    if !log_path.exists() {
        println!("No log file for step {}: {}", step_num, step_name);
        println!("The step may not have been executed yet.");
        return Ok(());
    }

    let content = fs::read_to_string(&log_path)?;
    print!("{}", content);

    Ok(())
}

/// Show all step logs in order
fn show_all_logs(project: &Project, task_name: &str) -> Result<()> {
    let log_dir = project.log_dir(task_name);
    let mut found_any = false;

    for (step_idx, step) in project.config.workflow.iter().enumerate() {
        let log_path = project.log_path(task_name, step_idx, &step.name);

        if log_path.exists() {
            if found_any {
                println!("\n{}\n", "=".repeat(60));
            }
            found_any = true;

            let content = fs::read_to_string(&log_path)?;
            print!("{}", content);
        }
    }

    if !found_any {
        println!("No log files found in {:?}", log_dir);
    }

    Ok(())
}
