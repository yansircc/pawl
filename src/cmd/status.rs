use anyhow::Result;
use chrono::Utc;

use crate::model::TaskStatus;

use super::common::Project;

/// Show status of all tasks or a specific task
pub fn run(task_name: Option<&str>) -> Result<()> {
    let project = Project::load()?;

    if let Some(name) = task_name {
        let name = project.resolve_task_name(name)?;
        show_task_detail(&project, &name)?;
    } else {
        show_all_tasks(&project)?;
    }

    Ok(())
}

/// List all tasks (alias for status without arguments)
pub fn list() -> Result<()> {
    run(None)
}

fn show_all_tasks(project: &Project) -> Result<()> {
    let tasks = project.load_all_tasks()?;

    if tasks.is_empty() {
        println!("No tasks found. Create one with: wf create <name>");
        return Ok(());
    }

    let workflow_len = project.config.workflow.len();

    // Header
    println!(
        "{:<15} {:<25} {:<12} {}",
        "NAME", "STEP", "STATUS", "INFO"
    );
    println!("{}", "-".repeat(70));

    for task_def in &tasks {
        let name = &task_def.name;

        let (step_str, status_str, info) = if let Some(state) = project.status.get(name) {
            let step_name = if state.current_step < workflow_len {
                &project.config.workflow[state.current_step].name
            } else {
                "Done"
            };
            let step_str = format!("[{}/{}] {}", state.current_step + 1, workflow_len, step_name);

            let status_str = format_status(state.status);

            let info = match state.status {
                TaskStatus::Running | TaskStatus::Waiting => {
                    format_duration(state.started_at)
                }
                TaskStatus::Failed => {
                    state.message.clone().unwrap_or_default()
                }
                TaskStatus::Pending => {
                    // Check dependencies
                    let blocking = project.check_dependencies(&task_def)?;
                    if !blocking.is_empty() {
                        format!("waiting: {}", blocking.join(", "))
                    } else {
                        String::new()
                    }
                }
                _ => String::new(),
            };

            (step_str, status_str, info)
        } else {
            // Task not started
            let blocking = project.check_dependencies(&task_def)?;
            let info = if !blocking.is_empty() {
                format!("waiting: {}", blocking.join(", "))
            } else {
                String::new()
            };
            ("--".to_string(), "pending".to_string(), info)
        };

        println!(
            "{:<15} {:<25} {:<12} {}",
            truncate(name, 14),
            truncate(&step_str, 24),
            status_str,
            truncate(&info, 25)
        );
    }

    Ok(())
}

fn show_task_detail(project: &Project, task_name: &str) -> Result<()> {
    let task_def = project.load_task(task_name)?;
    let workflow = &project.config.workflow;

    println!("Task: {}", task_name);
    println!();

    // Dependencies
    if !task_def.depends.is_empty() {
        println!("Dependencies: {}", task_def.depends.join(", "));
    }

    // Current state
    if let Some(state) = project.status.get(task_name) {
        println!("Status: {}", format_status(state.status));
        println!(
            "Step: {}/{}",
            state.current_step + 1,
            workflow.len()
        );

        if let Some(started) = state.started_at {
            println!("Started: {}", started.format("%Y-%m-%d %H:%M:%S"));
        }

        if let Some(msg) = &state.message {
            println!("Message: {}", msg);
        }

        println!();
    } else {
        println!("Status: not started");
        println!();
    }

    // Workflow steps
    println!("Workflow:");
    for (i, step) in workflow.iter().enumerate() {
        let current = project
            .status
            .get(task_name)
            .map(|s| s.current_step)
            .unwrap_or(0);

        let marker = if let Some(state) = project.status.get(task_name) {
            if i < current {
                if let Some(status) = state.step_status.get(&i) {
                    match status {
                        crate::model::StepStatus::Success => "✓",
                        crate::model::StepStatus::Failed => "✗",
                        crate::model::StepStatus::Skipped => "○",
                        crate::model::StepStatus::Blocked => "!",
                    }
                } else {
                    "✓"
                }
            } else if i == current {
                "→"
            } else {
                " "
            }
        } else {
            " "
        };

        let step_type = if step.is_checkpoint() {
            "(checkpoint)"
        } else if step.in_window {
            "(in_window)"
        } else {
            ""
        };

        println!("  {} {}. {} {}", marker, i + 1, step.name, step_type);
    }

    Ok(())
}

fn format_status(status: TaskStatus) -> String {
    match status {
        TaskStatus::Pending => "pending".to_string(),
        TaskStatus::Running => "running".to_string(),
        TaskStatus::Waiting => "waiting".to_string(),
        TaskStatus::Completed => "completed".to_string(),
        TaskStatus::Failed => "failed".to_string(),
        TaskStatus::Stopped => "stopped".to_string(),
    }
}

fn format_duration(started_at: Option<chrono::DateTime<Utc>>) -> String {
    let Some(started) = started_at else {
        return String::new();
    };

    let duration = Utc::now().signed_duration_since(started);
    let minutes = duration.num_minutes();
    let hours = duration.num_hours();

    if hours > 0 {
        format!("{}h {}m", hours, minutes % 60)
    } else if minutes > 0 {
        format!("{}m", minutes)
    } else {
        "< 1m".to_string()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
