use anyhow::Result;
use serde::Serialize;

use crate::model::TaskStatus;
use crate::util::tmux::{self, CaptureResult};

use super::common::Project;

#[derive(Serialize)]
struct CaptureOutput {
    task: String,
    session: String,
    window: String,
    window_exists: bool,
    process_active: bool,
    status: String,
    current_step: usize,
    step_name: String,
    lines: usize,
    content: String,
}

/// Capture tmux window content for a task
pub fn run(task_name: &str, lines: usize, json: bool) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    // Verify task exists
    let _task = project.load_task(&task_name)?;

    let session = project.session_name();
    let window = &task_name;

    // Get task status
    let (status, current_step, step_name) = if let Some(state) = project.status.get(&task_name) {
        let step_name = if state.current_step < project.config.workflow.len() {
            project.config.workflow[state.current_step].name.clone()
        } else {
            "Done".to_string()
        };
        (
            format!("{:?}", state.status).to_lowercase(),
            state.current_step + 1,
            step_name,
        )
    } else {
        ("pending".to_string(), 0, "--".to_string())
    };

    // Get task status to check for anomalies
    let task_status = project.status.get(&task_name).map(|s| s.status);

    // Capture content (also checks if window exists)
    let capture_result = tmux::capture_pane(&session, window, lines)?;

    let (window_exists, content) = match &capture_result {
        CaptureResult::Content(c) => (true, c.clone()),
        CaptureResult::WindowGone => (false, String::new()),
    };

    let process_active = if window_exists {
        tmux::pane_is_active(&session, window)
    } else {
        false
    };

    // Check for anomaly: task is running but window is gone
    let window_gone_warning = matches!(task_status, Some(TaskStatus::Running)) && !window_exists;

    if json {
        let output = CaptureOutput {
            task: task_name.clone(),
            session: session.clone(),
            window: window.clone(),
            window_exists,
            process_active,
            status,
            current_step,
            step_name,
            lines,
            content,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        // Human-readable output
        println!("Task: {}", task_name);
        println!("Window: {}:{}", session, window);
        println!("Status: {} (step {}: {})", status, current_step, step_name);
        println!(
            "Window: {} | Process: {}",
            if window_exists { "exists" } else { "not found" },
            if process_active { "active" } else { "idle" }
        );
        println!("{}", "=".repeat(60));

        if window_gone_warning {
            println!("WARNING: Task is running but tmux window is gone!");
            println!("         The task may have crashed or the window was killed.");
            println!("         Use 'wf retry {}' to restart the current step.", task_name);
        } else if window_exists {
            if content.is_empty() {
                println!("(no content)");
            } else {
                print!("{}", content);
            }
        } else {
            println!("Window does not exist. Task may not have started an in_window step yet.");
        }
    }

    Ok(())
}
