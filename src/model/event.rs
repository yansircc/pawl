use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::state::{StepStatus, TaskState, TaskStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    TaskStarted {
        ts: DateTime<Utc>,
    },
    StepCompleted {
        ts: DateTime<Utc>,
        step: usize,
        exit_code: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stdout: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stderr: Option<String>,
    },
    StepWaiting {
        ts: DateTime<Utc>,
        step: usize,
    },
    StepApproved {
        ts: DateTime<Utc>,
        step: usize,
    },
    WindowLaunched {
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
    VerifyFailed {
        ts: DateTime<Utc>,
        step: usize,
        feedback: String,
    },
    WindowLost {
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
            Event::StepCompleted { .. } => "step_completed",
            Event::StepWaiting { .. } => "step_waiting",
            Event::StepApproved { .. } => "step_approved",
            Event::WindowLaunched { .. } => "window_launched",
            Event::StepSkipped { .. } => "step_skipped",
            Event::StepReset { .. } => "step_reset",
            Event::TaskStopped { .. } => "task_stopped",
            Event::TaskReset { .. } => "task_reset",
            Event::VerifyFailed { .. } => "verify_failed",
            Event::WindowLost { .. } => "window_lost",
        }
    }

    /// Returns the step index associated with this event, if any
    pub fn step_index(&self) -> Option<usize> {
        match self {
            Event::TaskStarted { .. } | Event::TaskReset { .. } => None,
            Event::StepCompleted { step, .. }
            | Event::StepWaiting { step, .. }
            | Event::StepApproved { step, .. }
            | Event::WindowLaunched { step, .. }
            | Event::StepSkipped { step, .. }
            | Event::StepReset { step, .. }
            | Event::TaskStopped { step, .. }
            | Event::VerifyFailed { step, .. }
            | Event::WindowLost { step, .. } => Some(*step),
        }
    }

    /// Returns event-specific variables for hook template expansion
    pub fn extra_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        match self {
            Event::StepCompleted { exit_code, duration, .. } => {
                vars.insert("exit_code".to_string(), exit_code.to_string());
                if let Some(d) = duration {
                    vars.insert("duration".to_string(), format!("{:.1}", d));
                }
            }
            Event::StepReset { auto, .. } => {
                vars.insert("auto".to_string(), auto.to_string());
            }
            Event::VerifyFailed { feedback, .. } => {
                vars.insert("feedback".to_string(), feedback.clone());
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
            Event::TaskStarted { ts } => {
                state = Some(TaskState {
                    current_step: 0,
                    status: TaskStatus::Running,
                    started_at: Some(*ts),
                    updated_at: Some(*ts),
                    step_status: HashMap::new(),
                    message: None,
                });
            }
            Event::TaskReset { .. } => {
                state = None;
            }
            Event::StepCompleted {
                ts,
                step,
                exit_code,
                ..
            } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                if *exit_code == 0 {
                    s.step_status.insert(*step, StepStatus::Success);
                    s.current_step = step + 1;
                    s.status = TaskStatus::Running;
                    s.message = None;
                } else {
                    s.step_status.insert(*step, StepStatus::Failed);
                    s.status = TaskStatus::Failed;
                    s.message = Some(format!("Exit code: {}", exit_code));
                }
            }
            Event::StepWaiting { ts, .. } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.status = TaskStatus::Waiting;
            }
            Event::StepApproved { ts, step } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.step_status.insert(*step, StepStatus::Success);
                s.current_step = step + 1;
                s.status = TaskStatus::Running;
            }
            Event::WindowLaunched { ts, .. } => {
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
            Event::VerifyFailed { ts, step, feedback } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.step_status.insert(*step, StepStatus::Failed);
                s.status = TaskStatus::Failed;
                s.message = Some(feedback.clone());
            }
            Event::WindowLost { ts, step } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.step_status.insert(*step, StepStatus::Failed);
                s.status = TaskStatus::Failed;
                s.message = Some("tmux window lost".to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> DateTime<Utc> {
        Utc::now()
    }

    #[test]
    fn test_task_started() {
        let events = vec![Event::TaskStarted { ts: ts() }];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Running);
        assert_eq!(state.current_step, 0);
    }

    #[test]
    fn test_step_completed_success() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::StepCompleted {
                ts: ts(),
                step: 0,
                exit_code: 0,
                duration: Some(1.0),
                stdout: None,
                stderr: None,
            },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.current_step, 1);
        assert_eq!(state.status, TaskStatus::Running);
        assert_eq!(state.step_status.get(&0), Some(&StepStatus::Success));
    }

    #[test]
    fn test_step_completed_failure() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::StepCompleted {
                ts: ts(),
                step: 0,
                exit_code: 1,
                duration: Some(1.0),
                stdout: None,
                stderr: None,
            },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Failed);
        assert_eq!(state.step_status.get(&0), Some(&StepStatus::Failed));
    }

    #[test]
    fn test_step_waiting_approved() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::StepWaiting { ts: ts(), step: 0 },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Waiting);

        let mut events2 = events;
        events2.push(Event::StepApproved { ts: ts(), step: 0 });
        let state = replay(&events2, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Running);
        assert_eq!(state.current_step, 1);
    }

    #[test]
    fn test_verify_failed() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::VerifyFailed {
                ts: ts(),
                step: 0,
                feedback: "tests failed".to_string(),
            },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Failed);
        assert_eq!(state.step_status.get(&0), Some(&StepStatus::Failed));
        assert_eq!(state.message.as_deref(), Some("tests failed"));
    }

    #[test]
    fn test_auto_complete() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::StepCompleted {
                ts: ts(),
                step: 0,
                exit_code: 0,
                duration: Some(1.0),
                stdout: None,
                stderr: None,
            },
        ];
        let state = replay(&events, 1).unwrap();
        assert_eq!(state.status, TaskStatus::Completed);
    }

    #[test]
    fn test_reset_clears_state() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::StepCompleted {
                ts: ts(),
                step: 0,
                exit_code: 0,
                duration: Some(1.0),
                stdout: None,
                stderr: None,
            },
            Event::TaskReset { ts: ts() },
        ];
        let state = replay(&events, 3);
        assert!(state.is_none());
    }

    #[test]
    fn test_reset_then_restart() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::StepCompleted {
                ts: ts(),
                step: 0,
                exit_code: 0,
                duration: Some(1.0),
                stdout: None,
                stderr: None,
            },
            Event::TaskReset { ts: ts() },
            Event::TaskStarted { ts: ts() },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.current_step, 0);
        assert_eq!(state.status, TaskStatus::Running);
        assert!(state.step_status.is_empty());
    }

    #[test]
    fn test_skip_step() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::StepSkipped { ts: ts(), step: 0 },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.current_step, 1);
        assert_eq!(state.step_status.get(&0), Some(&StepStatus::Skipped));
    }

    #[test]
    fn test_task_stopped() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::TaskStopped { ts: ts(), step: 0 },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Stopped);
    }

    #[test]
    fn test_step_reset_auto() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::StepCompleted {
                ts: ts(),
                step: 0,
                exit_code: 1,
                duration: Some(1.0),
                stdout: None,
                stderr: None,
            },
            Event::StepReset {
                ts: ts(),
                step: 0,
                auto: true,
            },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.current_step, 0);
        assert_eq!(state.status, TaskStatus::Running);
        assert!(!state.step_status.contains_key(&0));
    }

    #[test]
    fn test_step_reset_manual() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::StepCompleted {
                ts: ts(),
                step: 0,
                exit_code: 0,
                duration: Some(1.0),
                stdout: None,
                stderr: None,
            },
            Event::StepReset {
                ts: ts(),
                step: 0,
                auto: false,
            },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.current_step, 0);
        assert_eq!(state.status, TaskStatus::Running);
    }

    #[test]
    fn test_window_lost() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::WindowLaunched { ts: ts(), step: 0 },
            Event::WindowLost { ts: ts(), step: 0 },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Failed);
        assert_eq!(state.step_status.get(&0), Some(&StepStatus::Failed));
        assert_eq!(state.message.as_deref(), Some("tmux window lost"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let event = Event::StepCompleted {
            ts: ts(),
            step: 0,
            exit_code: 0,
            duration: Some(5.2),
            stdout: Some("output".to_string()),
            stderr: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"step_completed""#));
        let _: Event = serde_json::from_str(&json).unwrap();
    }
}
