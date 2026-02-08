use anyhow::Result;
use serde::Serialize;

use crate::viewport::tmux::TmuxViewport;

use super::common::Project;

#[derive(Serialize)]
struct CaptureOutput {
    task: String,
    session: String,
    viewport_exists: bool,
    process_active: bool,
    status: String,
    current_step: usize,
    step_name: String,
    lines: usize,
    content: String,
}

/// Capture viewport content for a task
pub fn run(task_name: &str, lines: usize) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    // Verify task exists
    let _task = project.load_task(&task_name)?;

    let session = project.session_name();

    // Get task status via replay (auto-repairs viewport-lost)
    project.detect_viewport_loss(&task_name)?;
    let state = project.replay_task(&task_name)?;
    let (status, current_step, step_name) = if let Some(state) = &state {
        (
            state.status.to_string(),
            state.current_step,
            project.step_name(state.current_step).to_string(),
        )
    } else {
        ("pending".to_string(), 0, "--".to_string())
    };

    // Capture content (also checks if viewport exists)
    let content_opt = project.viewport.read(&task_name, lines)?;

    let viewport_exists = content_opt.is_some();
    let content = content_opt.unwrap_or_default();

    // pane_is_active is TmuxViewport-specific; downcast to check
    let process_active = if viewport_exists {
        if let Some(tmux_vp) = project.viewport.as_any().downcast_ref::<TmuxViewport>() {
            tmux_vp.pane_is_active(&task_name)
        } else {
            false
        }
    } else {
        false
    };

    let output = CaptureOutput {
        task: task_name.clone(),
        session: session.clone(),
        viewport_exists,
        process_active,
        status,
        current_step,
        step_name,
        lines,
        content,
    };
    println!("{}", serde_json::to_string(&output)?);

    Ok(())
}
