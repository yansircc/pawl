use anyhow::{bail, Result};
use std::io::{BufRead, BufReader};

use crate::model::Event;

use super::common::Project;

/// Show task logs
pub fn run(task_name: &str, step: Option<usize>, all: bool, all_runs: bool, jsonl: bool) -> Result<()> {
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
        if !jsonl {
            println!("No logs available for task '{}'.", task_name);
            println!("Logs are created when steps are executed.");
        }
        return Ok(());
    }

    if jsonl {
        return run_jsonl(&log_file, step, all, all_runs);
    }

    let events = project.read_events(&task_name)?;

    if events.is_empty() {
        println!("No log entries found for task '{}'.", task_name);
        return Ok(());
    }

    // Filter to current run unless --all-runs
    let events = if all_runs {
        events
    } else {
        current_run_events(events)
    };

    if all {
        for (i, event) in events.iter().enumerate() {
            if i > 0 {
                println!("\n{}", "\u{2500}".repeat(60));
            }
            print_event(event, &project);
        }
    } else if let Some(step_idx) = step {
        let matching: Vec<&Event> = events
            .iter()
            .filter(|e| e.step_index() == Some(step_idx))
            .collect();

        if matching.is_empty() {
            println!("No log entry found for step {}.", step_idx);
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

/// Filter events to only the current run (after the last TaskReset).
fn current_run_events(events: Vec<Event>) -> Vec<Event> {
    let last_reset_pos = events
        .iter()
        .rposition(|e| matches!(e, Event::TaskReset { .. }));

    match last_reset_pos {
        Some(pos) => events.into_iter().skip(pos + 1).collect(),
        None => events,
    }
}

/// Output raw JSONL lines, optionally filtered by step and/or current run
fn run_jsonl(log_file: &std::path::Path, step: Option<usize>, all: bool, all_runs: bool) -> Result<()> {
    let file = std::fs::File::open(log_file)?;
    let reader = BufReader::new(file);

    // Collect all lines
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
        Event::StepWaiting { ts, step, reason } => {
            let name = step_name(project, *step);
            println!("=== Step {}: {} (waiting) ===", step + 1, name);
            println!("Time: {}", ts.format("%Y-%m-%d %H:%M:%S"));
            println!("Waiting for approval ({}).", reason);
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
