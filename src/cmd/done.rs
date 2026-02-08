use anyhow::{bail, Result};

use crate::model::event::event_timestamp;
use crate::model::{Event, TaskStatus};

use super::common::Project;
use super::start;
use super::start::{resume_workflow, StepRecord};

/// Mark current step as done (approve waiting step, or complete in_viewport step)
pub fn done(task_name: &str, message: Option<&str>) -> Result<()> {
    let project = Project::load()?;
    let task_name = project.resolve_task_name(task_name)?;

    project.detect_viewport_loss(&task_name)?;

    let state = project.replay_task(&task_name)?;
    let Some(state) = state else {
        bail!("Task '{}' has not been started. Use 'pawl start {}' to begin.", task_name, task_name);
    };

    let step_idx = state.current_step;

    match state.status {
        TaskStatus::Running => {
            // Agent in viewport reporting done — go through unified pipeline
            let step = &project.config.workflow[step_idx];

            let record = StepRecord {
                exit_code: 0,
                duration: None,
                stdout: message.map(|s| s.to_string()),
                stderr: None,
            };

            eprintln!("Step {} marked as done.", step_idx + 1);

            // Unified pipeline: combine → decide → split
            let should_continue = start::settle_step(
                &project, &task_name, step_idx, step, record
            )?;

            if should_continue {
                resume_workflow(&project, &task_name)?;
            }

            // Cleanup viewport — but not if retrying
            let new_state = project.replay_task(&task_name)?;
            let retrying = matches!(&new_state,
                Some(s) if s.status == TaskStatus::Running && s.current_step == step_idx
            );
            if !retrying {
                let _ = project.viewport.close(&task_name);
            }
        }
        TaskStatus::Waiting => {
            // Human approval: emit StepResumed and continue
            project.append_event(&task_name, &Event::StepResumed {
                ts: event_timestamp(),
                step: step_idx,
            })?;

            eprintln!("Step {} approved.", step_idx + 1);
            resume_workflow(&project, &task_name)?;
        }
        _ => {
            bail!(
                "Task '{}' is {}. Cannot mark as done.",
                task_name,
                state.status
            );
        }
    }

    // Output final state as JSON
    project.output_task_state(&task_name)?;

    Ok(())
}
