use anyhow::Result;
use chrono::Utc;
use serde::Serialize;

use crate::model::{StepStatus, TaskStatus};
use crate::util::tmux;

use super::common::Project;

/// JSON output structure for task summary
#[derive(Serialize)]
struct TaskSummary {
    name: String,
    status: String,
    current_step: usize,
    total_steps: usize,
    step_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    blocked_by: Vec<String>,
}

/// JSON output structure for task detail
#[derive(Serialize)]
struct TaskDetail {
    name: String,
    description: Option<String>,
    depends: Vec<String>,
    status: String,
    current_step: usize,
    total_steps: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    workflow: Vec<StepInfo>,
}

#[derive(Serialize)]
struct StepInfo {
    index: usize,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    step_type: Option<String>,
    status: String,
}

/// Show status of all tasks or a specific task
pub fn run(task_name: Option<&str>, json: bool) -> Result<()> {
    let project = Project::load()?;

    if let Some(name) = task_name {
        let name = project.resolve_task_name(name)?;
        if json {
            show_task_detail_json(&project, &name)?;
        } else {
            show_task_detail(&project, &name)?;
        }
    } else {
        if json {
            show_all_tasks_json(&project)?;
        } else {
            show_all_tasks(&project)?;
        }
    }

    Ok(())
}

/// List all tasks (alias for status without arguments)
pub fn list(json: bool) -> Result<()> {
    run(None, json)
}

fn show_all_tasks_json(project: &Project) -> Result<()> {
    let tasks = project.load_all_tasks()?;
    let workflow_len = project.config.workflow.len();

    let mut summaries: Vec<TaskSummary> = Vec::new();

    for task_def in &tasks {
        let name = &task_def.name;
        let blocking = project.check_dependencies(&task_def)?;

        let summary = if let Some(state) = project.status.get(name) {
            let step_name = if state.current_step < workflow_len {
                project.config.workflow[state.current_step].name.clone()
            } else {
                "Done".to_string()
            };

            TaskSummary {
                name: name.clone(),
                status: format_status(state.status),
                current_step: state.current_step + 1,
                total_steps: workflow_len,
                step_name,
                message: state.message.clone(),
                started_at: state.started_at.map(|t| t.to_rfc3339()),
                updated_at: state.updated_at.map(|t| t.to_rfc3339()),
                blocked_by: blocking,
            }
        } else {
            TaskSummary {
                name: name.clone(),
                status: "pending".to_string(),
                current_step: 0,
                total_steps: workflow_len,
                step_name: "--".to_string(),
                message: None,
                started_at: None,
                updated_at: None,
                blocked_by: blocking,
            }
        };

        summaries.push(summary);
    }

    println!("{}", serde_json::to_string_pretty(&summaries)?);
    Ok(())
}

fn show_task_detail_json(project: &Project, task_name: &str) -> Result<()> {
    let task_def = project.load_task(task_name)?;
    let workflow = &project.config.workflow;
    let workflow_len = workflow.len();

    let state = project.status.get(task_name);
    let current_step = state.map(|s| s.current_step).unwrap_or(0);

    let mut steps: Vec<StepInfo> = Vec::new();
    for (i, step) in workflow.iter().enumerate() {
        let step_type = if step.is_checkpoint() {
            Some("checkpoint".to_string())
        } else if step.in_window {
            Some("in_window".to_string())
        } else {
            None
        };

        let step_status = if let Some(state) = state {
            if i < current_step {
                state
                    .step_status
                    .get(&i)
                    .map(|s| format_step_status(*s))
                    .unwrap_or_else(|| "success".to_string())
            } else if i == current_step {
                "current".to_string()
            } else {
                "pending".to_string()
            }
        } else {
            "pending".to_string()
        };

        steps.push(StepInfo {
            index: i + 1,
            name: step.name.clone(),
            step_type,
            status: step_status,
        });
    }

    let detail = TaskDetail {
        name: task_name.to_string(),
        description: if task_def.description.is_empty() {
            None
        } else {
            Some(task_def.description.clone())
        },
        depends: task_def.depends.clone(),
        status: state
            .map(|s| format_status(s.status))
            .unwrap_or_else(|| "pending".to_string()),
        current_step: current_step + 1,
        total_steps: workflow_len,
        message: state.and_then(|s| s.message.clone()),
        started_at: state.and_then(|s| s.started_at.map(|t| t.to_rfc3339())),
        updated_at: state.and_then(|s| s.updated_at.map(|t| t.to_rfc3339())),
        workflow: steps,
    };

    println!("{}", serde_json::to_string_pretty(&detail)?);
    Ok(())
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
                TaskStatus::Running => {
                    // Check if tmux window is alive
                    let session = project.session_name();
                    let window_alive = tmux::window_exists(&session, name);
                    if window_alive {
                        format_duration(state.started_at)
                    } else {
                        "!! window gone".to_string()
                    }
                }
                TaskStatus::Waiting => {
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

        // Check for anomaly: running but window gone
        if state.status == TaskStatus::Running {
            let session = project.session_name();
            if !tmux::window_exists(&session, task_name) {
                println!();
                println!("WARNING: Task is running but tmux window is gone!");
                println!("         The task may have crashed or the window was killed.");
                println!("         Use 'wf retry {}' to restart the current step.", task_name);
            }
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
                        StepStatus::Success => "✓",
                        StepStatus::Failed => "✗",
                        StepStatus::Skipped => "○",
                        StepStatus::Blocked => "!",
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

fn format_step_status(status: StepStatus) -> String {
    match status {
        StepStatus::Success => "success".to_string(),
        StepStatus::Failed => "failed".to_string(),
        StepStatus::Blocked => "blocked".to_string(),
        StepStatus::Skipped => "skipped".to_string(),
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
