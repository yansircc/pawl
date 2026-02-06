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
            .filter(|e| get_event_step(e) == Some(step_idx))
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

fn get_event_step(event: &Event) -> Option<usize> {
    match event {
        Event::TaskStarted { .. } => None,
        Event::TaskReset { .. } => None,
        Event::CommandExecuted { step, .. } => Some(*step),
        Event::CheckpointReached { step, .. } => Some(*step),
        Event::CheckpointPassed { step, .. } => Some(*step),
        Event::WindowLaunched { step, .. } => Some(*step),
        Event::AgentReported { step, .. } => Some(*step),
        Event::StepSkipped { step, .. } => Some(*step),
        Event::StepRetried { step, .. } => Some(*step),
        Event::StepRolledBack { from_step, .. } => Some(*from_step),
        Event::TaskStopped { step, .. } => Some(*step),
        Event::OnExit { step, .. } => Some(*step),
    }
}

fn print_event(event: &Event, project: &Project) {
    match event {
        Event::TaskStarted { ts } => {
            println!("=== Task Started ===");
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
        }
        Event::CommandExecuted {
            ts,
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
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
            println!("Exit code: {}", exit_code);
            println!("Duration: {:.1}s", duration);

            if !stdout.is_empty() {
                println!("\n[stdout]");
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
        Event::CheckpointReached { ts, step } => {
            let step_name = project
                .config
                .workflow
                .get(*step)
                .map(|s| s.name.as_str())
                .unwrap_or("Unknown");

            println!("=== Step {}: {} (checkpoint) ===", step + 1, step_name);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
            println!("Checkpoint reached.");
        }
        Event::CheckpointPassed { ts, step } => {
            let step_name = project
                .config
                .workflow
                .get(*step)
                .map(|s| s.name.as_str())
                .unwrap_or("Unknown");

            println!("=== Step {}: {} (checkpoint passed) ===", step + 1, step_name);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
        }
        Event::WindowLaunched { ts, step } => {
            let step_name = project
                .config
                .workflow
                .get(*step)
                .map(|s| s.name.as_str())
                .unwrap_or("Unknown");

            println!("=== Step {}: {} (window launched) ===", step + 1, step_name);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
        }
        Event::AgentReported {
            ts,
            step,
            result,
            session_id,
            transcript,
            message,
        } => {
            let step_name = project
                .config
                .workflow
                .get(*step)
                .map(|s| s.name.as_str())
                .unwrap_or("Unknown");

            println!("=== Step {}: {} (agent: {:?}) ===", step + 1, step_name, result);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));

            if let Some(sid) = session_id {
                println!("Session ID: {}", sid);
            }
            if let Some(path) = transcript {
                println!("Transcript: {}", path);
            }
            if let Some(msg) = message {
                println!("Message: {}", msg);
            }
        }
        Event::OnExit {
            ts,
            step,
            exit_code,
            session_id,
            transcript,
        } => {
            let step_name = project
                .config
                .workflow
                .get(*step)
                .map(|s| s.name.as_str())
                .unwrap_or("Unknown");

            println!("=== Step {}: {} (on_exit) ===", step + 1, step_name);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
            println!("Exit code: {}", exit_code);

            if let Some(sid) = session_id {
                println!("Session ID: {}", sid);
            }
            if let Some(path) = transcript {
                println!("Transcript: {}", path);
            }
        }
        Event::StepSkipped { ts, step } => {
            let step_name = project
                .config
                .workflow
                .get(*step)
                .map(|s| s.name.as_str())
                .unwrap_or("Unknown");

            println!("=== Step {}: {} (skipped) ===", step + 1, step_name);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
        }
        Event::StepRetried { ts, step } => {
            let step_name = project
                .config
                .workflow
                .get(*step)
                .map(|s| s.name.as_str())
                .unwrap_or("Unknown");

            println!("=== Step {}: {} (retried) ===", step + 1, step_name);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
        }
        Event::StepRolledBack {
            ts,
            from_step,
            to_step,
        } => {
            println!("=== Rolled back: step {} → step {} ===", from_step + 1, to_step + 1);
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
    }
}
