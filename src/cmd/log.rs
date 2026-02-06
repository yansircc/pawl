use anyhow::{bail, Result};

use crate::model::Event;

use super::common::Project;

/// Show task logs
pub fn run(task_name: &str, step: Option<usize>, all: bool) -> Result<()> {
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
        println!("No logs available for task '{}'.", task_name);
        println!("Logs are created when steps are executed.");
        return Ok(());
    }

    let events = project.read_events(&task_name)?;

    if events.is_empty() {
        println!("No log entries found for task '{}'.", task_name);
        return Ok(());
    }

    if all {
        for (i, event) in events.iter().enumerate() {
            if i > 0 {
                println!("\n{}", "\u{2500}".repeat(60));
            }
            print_event(event, &project);
        }
    } else if let Some(step_num) = step {
        let step_idx = step_num.saturating_sub(1);

        let matching: Vec<&Event> = events
            .iter()
            .filter(|e| e.step_index() == Some(step_idx))
            .collect();

        if matching.is_empty() {
            println!("No log entry found for step {}.", step_num);
            println!("The step may not have been executed yet.");
        } else {
            for (i, event) in matching.iter().enumerate() {
                if i > 0 {
                    println!("\n{}", "\u{2500}".repeat(60));
                }
                print_event(event, &project);
            }
        }
    } else {
        // Show the most recent event
        if let Some(event) = events.last() {
            print_event(event, &project);
        } else {
            println!("No log entries found.");
        }
    }

    Ok(())
}

fn step_name(project: &Project, step: usize) -> String {
    match project.config.workflow.get(step) {
        Some(s) => s.name.clone(),
        None => {
            eprintln!("Warning: step index {} out of range (workflow has {} steps), event log may be corrupted",
                step, project.config.workflow.len());
            format!("step_{}", step)
        }
    }
}

fn print_event(event: &Event, project: &Project) {
    match event {
        Event::TaskStarted { ts } => {
            println!("=== Task Started ===");
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
        }
        Event::StepCompleted {
            ts,
            step,
            exit_code,
            duration,
            stdout,
            stderr,
        } => {
            let name = step_name(project, *step);
            println!("=== Step {}: {} (completed, exit {}) ===", step + 1, name, exit_code);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
            if let Some(d) = duration {
                println!("Duration: {:.1}s", d);
            }

            if let Some(out) = stdout {
                if !out.is_empty() {
                    println!("\n[stdout]");
                    let lines: Vec<&str> = out.lines().collect();
                    if lines.len() > 50 {
                        for line in lines.iter().take(25) {
                            println!("{}", line);
                        }
                        println!("... ({} lines omitted) ...", lines.len() - 50);
                        for line in lines.iter().skip(lines.len() - 25) {
                            println!("{}", line);
                        }
                    } else {
                        print!("{}", out);
                        if !out.ends_with('\n') {
                            println!();
                        }
                    }
                }
            }

            if let Some(err) = stderr {
                if !err.is_empty() {
                    println!("\n[stderr]");
                    print!("{}", err);
                    if !err.ends_with('\n') {
                        println!();
                    }
                }
            }
        }
        Event::StepWaiting { ts, step } => {
            let name = step_name(project, *step);
            println!("=== Step {}: {} (waiting) ===", step + 1, name);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
            println!("Waiting for approval.");
        }
        Event::StepApproved { ts, step } => {
            let name = step_name(project, *step);
            println!("=== Step {}: {} (approved) ===", step + 1, name);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
        }
        Event::WindowLaunched { ts, step } => {
            let name = step_name(project, *step);
            println!("=== Step {}: {} (window launched) ===", step + 1, name);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
        }
        Event::StepSkipped { ts, step } => {
            let name = step_name(project, *step);
            println!("=== Step {}: {} (skipped) ===", step + 1, name);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
        }
        Event::StepReset { ts, step, auto } => {
            let name = step_name(project, *step);
            let mode = if *auto { "auto" } else { "manual" };
            println!("=== Step {}: {} (reset, {}) ===", step + 1, name, mode);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
        }
        Event::TaskStopped { ts, step } => {
            println!("=== Task stopped at step {} ===", step + 1);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
        }
        Event::TaskReset { ts } => {
            println!("=== Task Reset ===");
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
        }
        Event::VerifyFailed { ts, step, feedback } => {
            let name = step_name(project, *step);
            println!("=== Step {}: {} (verify failed) ===", step + 1, name);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
            if !feedback.is_empty() {
                println!("\n[feedback]");
                println!("{}", feedback);
            }
        }
        Event::WindowLost { ts, step } => {
            let name = step_name(project, *step);
            println!("=== Step {}: {} (window lost) ===", step + 1, name);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
            println!("tmux window disappeared — auto-marked as failed.");
        }
    }
}
