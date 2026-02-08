use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::state::{StepStatus, TaskState, TaskStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    TaskStarted {
        ts: DateTime<Utc>,
        run_id: String,
    },
    StepFinished {
        ts: DateTime<Utc>,
        step: usize,
        success: bool,
        exit_code: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stdout: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stderr: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        verify_output: Option<String>,
    },
    StepYielded {
        ts: DateTime<Utc>,
        step: usize,
        reason: String,
    },
    StepResumed {
        ts: DateTime<Utc>,
        step: usize,
    },
    ViewportLaunched {
        ts: DateTime<Utc>,
        step: usize,
    },
    StepSkipped {
        ts: DateTime<Utc>,
        step: usize,
    },
    StepReset {
        ts: DateTime<Utc>,
        step: usize,
        auto: bool,
    },
    TaskStopped {
        ts: DateTime<Utc>,
        step: usize,
    },
    TaskReset {
        ts: DateTime<Utc>,
    },
    ViewportLost {
        ts: DateTime<Utc>,
        step: usize,
    },
}

pub fn event_timestamp() -> DateTime<Utc> {
    Utc::now()
}

impl Event {
    /// Returns the serde snake_case tag name for this event
    pub fn type_name(&self) -> &'static str {
        match self {
            Event::TaskStarted { .. } => "task_started",
            Event::StepFinished { .. } => "step_finished",
            Event::StepYielded { .. } => "step_yielded",
            Event::StepResumed { .. } => "step_resumed",
            Event::ViewportLaunched { .. } => "viewport_launched",
            Event::StepSkipped { .. } => "step_skipped",
            Event::StepReset { .. } => "step_reset",
            Event::TaskStopped { .. } => "task_stopped",
            Event::TaskReset { .. } => "task_reset",
            Event::ViewportLost { .. } => "viewport_lost",
        }
    }

    /// Returns the step index associated with this event, if any
    pub fn step_index(&self) -> Option<usize> {
        match self {
            Event::TaskStarted { .. } | Event::TaskReset { .. } => None,
            Event::StepFinished { step, .. }
            | Event::StepYielded { step, .. }
            | Event::StepResumed { step, .. }
            | Event::ViewportLaunched { step, .. }
            | Event::StepSkipped { step, .. }
            | Event::StepReset { step, .. }
            | Event::TaskStopped { step, .. }
            | Event::ViewportLost { step, .. } => Some(*step),
        }
    }

    /// Returns event-specific variables for hook template expansion
    pub fn extra_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        match self {
            Event::StepFinished { success, exit_code, duration, .. } => {
                vars.insert("success".to_string(), success.to_string());
                vars.insert("exit_code".to_string(), exit_code.to_string());
                if let Some(d) = duration {
                    vars.insert("duration".to_string(), format!("{:.1}", d));
                }
            }
            Event::TaskStarted { run_id, .. } => {
                vars.insert("run_id".to_string(), run_id.clone());
            }
            Event::StepYielded { reason, .. } => {
                vars.insert("reason".to_string(), reason.clone());
            }
            Event::StepReset { auto, .. } => {
                vars.insert("auto".to_string(), auto.to_string());
            }
            _ => {}
        }
        vars
    }
}

/// Replay events to reconstruct TaskState.
/// Returns None if no TaskStarted event found (after last reset).
pub fn replay(events: &[Event], workflow_len: usize) -> Option<TaskState> {
    let mut state: Option<TaskState> = None;

    for event in events {
        match event {
            Event::TaskStarted { ts, run_id } => {
                state = Some(TaskState {
                    current_step: 0,
                    status: TaskStatus::Running,
                    started_at: Some(*ts),
                    updated_at: Some(*ts),
                    step_status: HashMap::new(),
                    message: None,
                    run_id: run_id.clone(),
                });
            }
            Event::TaskReset { .. } => {
                state = None;
            }
            Event::StepFinished {
                ts,
                step,
                success,
                ..
            } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                if *success {
                    s.step_status.insert(*step, StepStatus::Success);
                    s.current_step = step + 1;
                    s.status = TaskStatus::Running;
                    s.message = None;
                } else {
                    s.step_status.insert(*step, StepStatus::Failed);
                    s.status = TaskStatus::Failed;
                    s.message = None;
                }
            }
            Event::StepYielded { ts, step, reason } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.current_step = *step;
                s.status = TaskStatus::Waiting;
                s.message = Some(reason.clone());
            }
            Event::StepResumed { ts, step } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.step_status.insert(*step, StepStatus::Success);
                s.current_step = step + 1;
                s.status = TaskStatus::Running;
            }
            Event::ViewportLaunched { ts, .. } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.status = TaskStatus::Running;
            }
            Event::StepSkipped { ts, step } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.step_status.insert(*step, StepStatus::Skipped);
                s.current_step = step + 1;
                s.status = TaskStatus::Running;
            }
            Event::StepReset { ts, step, .. } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.current_step = *step;
                s.step_status.remove(step);
                s.status = TaskStatus::Running;
                s.message = None;
            }
            Event::TaskStopped { ts, .. } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.status = TaskStatus::Stopped;
            }
            Event::ViewportLost { ts, step } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.step_status.insert(*step, StepStatus::Failed);
                s.status = TaskStatus::Failed;
                s.message = Some("viewport lost".to_string());
            }
        }
    }

    // Auto-derive Completed when all steps done
    if let Some(s) = state.as_mut() {
        if s.current_step >= workflow_len && s.status == TaskStatus::Running {
            s.status = TaskStatus::Completed;
        }
    }

    state
}

/// Count auto-retries for a specific step since last TaskStarted/TaskReset(manual).
pub fn count_auto_retries(events: &[Event], step_idx: usize) -> usize {
    let mut count = 0;
    for event in events.iter().rev() {
        match event {
            Event::TaskStarted { .. } | Event::TaskReset { .. } => break,
            Event::StepReset { step, auto: false, .. } if *step == step_idx => break,
            Event::StepReset { step, auto: true, .. } if *step == step_idx => {
                count += 1;
            }
            _ => {}
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> DateTime<Utc> {
        Utc::now()
    }

    fn finished(step: usize, success: bool, exit_code: i32) -> Event {
        Event::StepFinished {
            ts: ts(), step, success, exit_code,
            duration: Some(1.0), stdout: None, stderr: None, verify_output: None,
        }
    }

    #[test]
    fn test_task_started() {
        let events = vec![Event::TaskStarted { ts: ts(), run_id: String::new() }];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Running);
        assert_eq!(state.current_step, 0);
    }

    #[test]
    fn test_step_finished_success() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            finished(0, true, 0),
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.current_step, 1);
        assert_eq!(state.status, TaskStatus::Running);
        assert_eq!(state.step_status.get(&0), Some(&StepStatus::Success));
    }

    #[test]
    fn test_step_finished_failure() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            finished(0, false, 1),
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Failed);
        assert_eq!(state.step_status.get(&0), Some(&StepStatus::Failed));
    }

    #[test]
    fn test_step_yielded_resumed() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            Event::StepYielded { ts: ts(), step: 0, reason: "gate".to_string() },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Waiting);
        assert_eq!(state.message.as_deref(), Some("gate"));

        let mut events2 = events;
        events2.push(Event::StepResumed { ts: ts(), step: 0 });
        let state = replay(&events2, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Running);
        assert_eq!(state.current_step, 1);
    }

    #[test]
    fn test_step_yielded_after_finished_resets_current_step() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            finished(0, true, 0),
            Event::StepYielded { ts: ts(), step: 0, reason: "verify_human".to_string() },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Waiting);
        assert_eq!(state.current_step, 0);
    }

    #[test]
    fn test_verify_failure_as_step_finished() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            Event::StepFinished {
                ts: ts(), step: 0, success: false, exit_code: 0,
                duration: Some(2.0), stdout: None, stderr: None,
                verify_output: Some("verify: tests failed".to_string()),
            },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Failed);
        assert_eq!(state.current_step, 0);
        assert_eq!(state.step_status.get(&0), Some(&StepStatus::Failed));
    }

    #[test]
    fn test_verify_failure_then_retry() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            finished(0, false, 1),
            Event::StepReset { ts: ts(), step: 0, auto: true },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Running);
        assert_eq!(state.current_step, 0);
        assert!(!state.step_status.contains_key(&0));
    }

    #[test]
    fn test_auto_complete() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            finished(0, true, 0),
        ];
        let state = replay(&events, 1).unwrap();
        assert_eq!(state.status, TaskStatus::Completed);
    }

    #[test]
    fn test_reset_clears_state() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            finished(0, true, 0),
            Event::TaskReset { ts: ts() },
        ];
        let state = replay(&events, 3);
        assert!(state.is_none());
    }

    #[test]
    fn test_reset_then_restart() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            finished(0, true, 0),
            Event::TaskReset { ts: ts() },
            Event::TaskStarted { ts: ts(), run_id: String::new() },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.current_step, 0);
        assert_eq!(state.status, TaskStatus::Running);
        assert!(state.step_status.is_empty());
    }

    #[test]
    fn test_skip_step() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            Event::StepSkipped { ts: ts(), step: 0 },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.current_step, 1);
        assert_eq!(state.step_status.get(&0), Some(&StepStatus::Skipped));
    }

    #[test]
    fn test_task_stopped() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            Event::TaskStopped { ts: ts(), step: 0 },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Stopped);
    }

    #[test]
    fn test_step_reset_auto() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            finished(0, false, 1),
            Event::StepReset { ts: ts(), step: 0, auto: true },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.current_step, 0);
        assert_eq!(state.status, TaskStatus::Running);
        assert!(!state.step_status.contains_key(&0));
    }

    #[test]
    fn test_step_reset_manual() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            finished(0, true, 0),
            Event::StepReset { ts: ts(), step: 0, auto: false },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.current_step, 0);
        assert_eq!(state.status, TaskStatus::Running);
    }

    #[test]
    fn test_viewport_lost() {
        let events = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            Event::ViewportLaunched { ts: ts(), step: 0 },
            Event::ViewportLost { ts: ts(), step: 0 },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Failed);
        assert_eq!(state.step_status.get(&0), Some(&StepStatus::Failed));
        assert_eq!(state.message.as_deref(), Some("viewport lost"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let event = Event::StepFinished {
            ts: ts(), step: 0, success: true, exit_code: 0,
            duration: Some(5.2), stdout: Some("output".to_string()),
            stderr: None, verify_output: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"step_finished""#));
        let _: Event = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_type_name_matches_serde_tag() {
        let events: Vec<Event> = vec![
            Event::TaskStarted { ts: ts(), run_id: String::new() },
            Event::StepFinished {
                ts: ts(), step: 0, success: true, exit_code: 0,
                duration: None, stdout: None, stderr: None, verify_output: None,
            },
            Event::StepYielded { ts: ts(), step: 0, reason: "gate".to_string() },
            Event::StepResumed { ts: ts(), step: 0 },
            Event::ViewportLaunched { ts: ts(), step: 0 },
            Event::StepSkipped { ts: ts(), step: 0 },
            Event::StepReset { ts: ts(), step: 0, auto: false },
            Event::TaskStopped { ts: ts(), step: 0 },
            Event::TaskReset { ts: ts() },
            Event::ViewportLost { ts: ts(), step: 0 },
        ];
        for event in &events {
            let json: serde_json::Value = serde_json::to_value(event).unwrap();
            let serde_tag = json.get("type").unwrap().as_str().unwrap();
            assert_eq!(
                event.type_name(), serde_tag,
                "type_name() mismatch for {:?}", event.type_name()
            );
        }
    }
}
