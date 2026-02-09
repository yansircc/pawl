use anyhow::Result;
use serde::Serialize;

use super::common::{extract_step_context, Project};

/// JSON output structure for task summary
#[derive(Serialize)]
struct TaskSummary {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    suggest: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    suggest: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
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

/// Derive routing hints from task status. Only used by status output.
/// suggest = mechanical commands, prompt = requires judgment.
fn derive_routing(status: &str, message: Option<&str>, task: &str) -> (Vec<String>, Option<String>) {
    match status {
        "pending" => (vec![format!("pawl start {task}")], None),
        "waiting" => match message {
            Some("gate") => (
                vec![],
                Some(format!("confirm preconditions, then: pawl done {task}")),
            ),
            Some("verify_manual") => (
                vec![],
                Some(format!("verify work quality, then: pawl done {task}")),
            ),
            Some("on_fail_manual") => (
                vec![format!("pawl reset --step {task}")],
                Some(format!("review failure, then: pawl done {task} to accept")),
            ),
            _ => (vec![], None),
        },
        "failed" => (vec![format!("pawl reset --step {task}")], None),
        "stopped" => (
            vec![format!("pawl start {task}"), format!("pawl reset {task}")],
            None,
        ),
        _ => (vec![], None),
    }
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
    let tasks = project.discover_tasks()?;
    let workflow_len = project.config.workflow.len();

    let mut summaries: Vec<TaskSummary> = Vec::new();

    for name in &tasks {
        let tc = project.task_config(name);
        let blocking = project.check_dependencies(name)?;
        let description = tc.and_then(|t| t.description.clone());

        project.detect_viewport_loss(name)?;
        let summary = if let Some(state) = project.replay_task(name)? {
            let step_name = project.step_name(state.current_step).to_string();
            let events = project.read_events(name)?;
            let (retry_count, last_feedback) = extract_step_context(&events, state.current_step);
            let status_str = state.status.to_string();
            let (suggest, prompt) = derive_routing(&status_str, state.message.as_deref(), name);

            TaskSummary {
                name: name.clone(),
                description,
                status: status_str,
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
                suggest,
                prompt,
            }
        } else {
            let (suggest, prompt) = derive_routing("pending", None, name);
            TaskSummary {
                name: name.clone(),
                description,
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
                suggest,
                prompt,
            }
        };

        summaries.push(summary);
    }

    println!("{}", serde_json::to_string(&summaries)?);
    Ok(())
}

fn show_task_detail(project: &Project, task_name: &str) -> Result<()> {
    let tc = project.task_config(task_name);
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

    let status_str = state
        .as_ref()
        .map(|s| s.status.to_string())
        .unwrap_or_else(|| "pending".to_string());
    let msg = state.as_ref().and_then(|s| s.message.clone());
    let (suggest, prompt) = derive_routing(&status_str, msg.as_deref(), task_name);

    let detail = TaskDetail {
        name: task_name.to_string(),
        description: tc.and_then(|t| t.description.clone()),
        depends: tc.map(|t| t.depends.clone()).unwrap_or_default(),
        status: status_str,
        run_id: state.as_ref().map(|s| s.run_id.clone()).unwrap_or_default(),
        current_step,
        total_steps: workflow_len,
        message: msg,
        started_at: state.as_ref().and_then(|s| s.started_at.map(|t| t.to_rfc3339())),
        updated_at: state.as_ref().and_then(|s| s.updated_at.map(|t| t.to_rfc3339())),
        retry_count,
        last_feedback,
        suggest,
        prompt,
        workflow: steps,
    };

    println!("{}", serde_json::to_string(&detail)?);
    Ok(())
}
