use anyhow::Result;
use serde::Serialize;

use super::common::{extract_step_context, Project};

/// JSON output structure for task summary
#[derive(Serialize)]
struct TaskSummary {
    name: String,
    status: String,
    run_id: String,
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
    retry_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_feedback: Option<String>,
}

/// JSON output structure for task detail
#[derive(Serialize)]
struct TaskDetail {
    name: String,
    description: Option<String>,
    depends: Vec<String>,
    status: String,
    run_id: String,
    current_step: usize,
    total_steps: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    retry_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_feedback: Option<String>,
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
    let workflow_len = project.config.workflow.len();

    let mut summaries: Vec<TaskSummary> = Vec::new();

    for task_def in &tasks {
        let name = &task_def.name;
        let blocking = project.check_dependencies(task_def)?;

        project.detect_viewport_loss(name)?;
        let summary = if let Some(state) = project.replay_task(name)? {
            let step_name = project.step_name(state.current_step).to_string();
            let events = project.read_events(name)?;
            let (retry_count, last_feedback) = extract_step_context(&events, state.current_step);

            TaskSummary {
                name: name.clone(),
                status: state.status.to_string(),
                run_id: state.run_id,
                current_step: state.current_step,
                total_steps: workflow_len,
                step_name,
                message: state.message.clone(),
                started_at: state.started_at.map(|t| t.to_rfc3339()),
                updated_at: state.updated_at.map(|t| t.to_rfc3339()),
                blocked_by: blocking,
                retry_count,
                last_feedback,
            }
        } else {
            TaskSummary {
                name: name.clone(),
                status: "pending".to_string(),
                run_id: String::new(),
                current_step: 0,
                total_steps: workflow_len,
                step_name: "--".to_string(),
                message: None,
                started_at: None,
                updated_at: None,
                blocked_by: blocking,
                retry_count: 0,
                last_feedback: None,
            }
        };

        summaries.push(summary);
    }

    println!("{}", serde_json::to_string(&summaries)?);
    Ok(())
}

fn show_task_detail(project: &Project, task_name: &str) -> Result<()> {
    let task_def = project.load_task(task_name)?;
    let workflow = &project.config.workflow;
    let workflow_len = workflow.len();

    project.detect_viewport_loss(task_name)?;
    let state = project.replay_task(task_name)?;
    let current_step = state.as_ref().map(|s| s.current_step).unwrap_or(0);

    let mut steps: Vec<StepInfo> = Vec::new();
    for (i, step) in workflow.iter().enumerate() {
        let step_type = if step.is_gate() {
            Some("gate".to_string())
        } else if step.in_viewport {
            Some("in_viewport".to_string())
        } else {
            None
        };

        let step_status = if let Some(state) = &state {
            if i < current_step {
                state
                    .step_status
                    .get(&i)
                    .map(|s| s.to_string())
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
            index: i,
            name: step.name.clone(),
            step_type,
            status: step_status,
        });
    }

    let events = project.read_events(task_name)?;
    let (retry_count, last_feedback) = extract_step_context(&events, current_step);

    let detail = TaskDetail {
        name: task_name.to_string(),
        description: if task_def.description.is_empty() {
            None
        } else {
            Some(task_def.description.clone())
        },
        depends: task_def.depends.clone(),
        status: state
            .as_ref()
            .map(|s| s.status.to_string())
            .unwrap_or_else(|| "pending".to_string()),
        run_id: state.as_ref().map(|s| s.run_id.clone()).unwrap_or_default(),
        current_step,
        total_steps: workflow_len,
        message: state.as_ref().and_then(|s| s.message.clone()),
        started_at: state.as_ref().and_then(|s| s.started_at.map(|t| t.to_rfc3339())),
        updated_at: state.as_ref().and_then(|s| s.updated_at.map(|t| t.to_rfc3339())),
        retry_count,
        last_feedback,
        workflow: steps,
    };

    println!("{}", serde_json::to_string(&detail)?);
    Ok(())
}
