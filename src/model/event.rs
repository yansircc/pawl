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
    CommandExecuted {
        ts: DateTime<Utc>,
        step: usize,
        exit_code: i32,
        duration: f64,
        stdout: String,
        stderr: String,
    },
    CheckpointReached {
        ts: DateTime<Utc>,
        step: usize,
    },
    CheckpointPassed {
        ts: DateTime<Utc>,
        step: usize,
    },
    WindowLaunched {
        ts: DateTime<Utc>,
        step: usize,
    },
    AgentReported {
        ts: DateTime<Utc>,
        step: usize,
        result: AgentResult,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transcript: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    StepSkipped {
        ts: DateTime<Utc>,
        step: usize,
    },
    StepRetried {
        ts: DateTime<Utc>,
        step: usize,
    },
    StepRolledBack {
        ts: DateTime<Utc>,
        from_step: usize,
        to_step: usize,
    },
    TaskStopped {
        ts: DateTime<Utc>,
        step: usize,
    },
    TaskReset {
        ts: DateTime<Utc>,
    },
    OnExit {
        ts: DateTime<Utc>,
        step: usize,
        exit_code: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transcript: Option<String>,
    },
    WindowLost {
        ts: DateTime<Utc>,
        step: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentResult {
    Done,
    Failed,
    Blocked,
}

pub fn event_timestamp() -> DateTime<Utc> {
    Utc::now()
}

impl Event {
    /// Returns the serde snake_case tag name for this event
    pub fn type_name(&self) -> &'static str {
        match self {
            Event::TaskStarted { .. } => "task_started",
            Event::CommandExecuted { .. } => "command_executed",
            Event::CheckpointReached { .. } => "checkpoint_reached",
            Event::CheckpointPassed { .. } => "checkpoint_passed",
            Event::WindowLaunched { .. } => "window_launched",
            Event::AgentReported { .. } => "agent_reported",
            Event::OnExit { .. } => "on_exit",
            Event::StepSkipped { .. } => "step_skipped",
            Event::StepRetried { .. } => "step_retried",
            Event::StepRolledBack { .. } => "step_rolled_back",
            Event::TaskStopped { .. } => "task_stopped",
            Event::TaskReset { .. } => "task_reset",
            Event::WindowLost { .. } => "window_lost",
        }
    }

    /// Returns the step index associated with this event, if any
    pub fn step_index(&self) -> Option<usize> {
        match self {
            Event::TaskStarted { .. } | Event::TaskReset { .. } => None,
            Event::CommandExecuted { step, .. }
            | Event::CheckpointReached { step, .. }
            | Event::CheckpointPassed { step, .. }
            | Event::WindowLaunched { step, .. }
            | Event::AgentReported { step, .. }
            | Event::OnExit { step, .. }
            | Event::StepSkipped { step, .. }
            | Event::StepRetried { step, .. }
            | Event::TaskStopped { step, .. }
            | Event::WindowLost { step, .. } => Some(*step),
            Event::StepRolledBack { from_step, .. } => Some(*from_step),
        }
    }

    /// Returns event-specific variables for hook template expansion
    pub fn extra_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        match self {
            Event::CommandExecuted { exit_code, duration, .. } => {
                vars.insert("exit_code".to_string(), exit_code.to_string());
                vars.insert("duration".to_string(), format!("{:.1}", duration));
            }
            Event::AgentReported { result, message, session_id, .. } => {
                vars.insert("result".to_string(), format!("{:?}", result).to_lowercase());
                if let Some(msg) = message {
                    vars.insert("message".to_string(), msg.clone());
                }
                if let Some(sid) = session_id {
                    vars.insert("session_id".to_string(), sid.clone());
                }
            }
            Event::OnExit { exit_code, session_id, .. } => {
                vars.insert("exit_code".to_string(), exit_code.to_string());
                if let Some(sid) = session_id {
                    vars.insert("session_id".to_string(), sid.clone());
                }
            }
            Event::StepRolledBack { from_step, to_step, .. } => {
                vars.insert("from_step".to_string(), from_step.to_string());
                vars.insert("to_step".to_string(), to_step.to_string());
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
            Event::CommandExecuted {
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
            Event::CheckpointReached { ts, .. } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.status = TaskStatus::Waiting;
            }
            Event::CheckpointPassed { ts, step } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.current_step = step + 1;
                s.status = TaskStatus::Running;
            }
            Event::WindowLaunched { ts, .. } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.status = TaskStatus::Running;
            }
            Event::AgentReported {
                ts,
                step,
                result,
                message,
                ..
            } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                match result {
                    AgentResult::Done => {
                        s.step_status.insert(*step, StepStatus::Success);
                        s.current_step = step + 1;
                        s.status = TaskStatus::Running;
                        s.message = message.clone();
                    }
                    AgentResult::Failed => {
                        s.step_status.insert(*step, StepStatus::Failed);
                        s.status = TaskStatus::Failed;
                        s.message = message.clone();
                    }
                    AgentResult::Blocked => {
                        s.step_status.insert(*step, StepStatus::Blocked);
                        s.status = TaskStatus::Waiting;
                        s.message = message.clone();
                    }
                }
            }
            Event::OnExit {
                ts,
                step,
                exit_code,
                ..
            } => {
                let Some(s) = state.as_mut() else { continue };
                // Only apply if this step hasn't been handled by AgentReported
                if s.step_status.contains_key(step) {
                    continue;
                }
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
            Event::StepSkipped { ts, step } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.step_status.insert(*step, StepStatus::Skipped);
                s.current_step = step + 1;
                s.status = TaskStatus::Running;
            }
            Event::StepRetried { ts, .. } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.status = TaskStatus::Running;
                s.message = None;
            }
            Event::StepRolledBack { ts, to_step, .. } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.current_step = *to_step;
                s.status = TaskStatus::Waiting;
                s.message = None;
            }
            Event::TaskStopped { ts, .. } => {
                let Some(s) = state.as_mut() else { continue };
                s.updated_at = Some(*ts);
                s.status = TaskStatus::Stopped;
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
    fn test_command_success_advances() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::CommandExecuted {
                ts: ts(),
                step: 0,
                exit_code: 0,
                duration: 1.0,
                stdout: String::new(),
                stderr: String::new(),
            },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.current_step, 1);
        assert_eq!(state.status, TaskStatus::Running);
        assert_eq!(state.step_status.get(&0), Some(&StepStatus::Success));
    }

    #[test]
    fn test_command_failure() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::CommandExecuted {
                ts: ts(),
                step: 0,
                exit_code: 1,
                duration: 1.0,
                stdout: String::new(),
                stderr: String::new(),
            },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Failed);
        assert_eq!(state.step_status.get(&0), Some(&StepStatus::Failed));
    }

    #[test]
    fn test_checkpoint_flow() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::CheckpointReached { ts: ts(), step: 0 },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Waiting);

        let mut events2 = events;
        events2.push(Event::CheckpointPassed { ts: ts(), step: 0 });
        let state = replay(&events2, 3).unwrap();
        assert_eq!(state.status, TaskStatus::Running);
        assert_eq!(state.current_step, 1);
    }

    #[test]
    fn test_agent_done() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::WindowLaunched { ts: ts(), step: 0 },
            Event::AgentReported {
                ts: ts(),
                step: 0,
                result: AgentResult::Done,
                session_id: None,
                transcript: None,
                message: None,
            },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.current_step, 1);
        assert_eq!(state.step_status.get(&0), Some(&StepStatus::Success));
    }

    #[test]
    fn test_on_exit_ignored_after_agent_reported() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::WindowLaunched { ts: ts(), step: 0 },
            Event::AgentReported {
                ts: ts(),
                step: 0,
                result: AgentResult::Done,
                session_id: None,
                transcript: None,
                message: None,
            },
            Event::OnExit {
                ts: ts(),
                step: 0,
                exit_code: 0,
                session_id: None,
                transcript: None,
            },
        ];
        let state = replay(&events, 3).unwrap();
        // Should be at step 1, not step 2 (OnExit should be ignored)
        assert_eq!(state.current_step, 1);
    }

    #[test]
    fn test_auto_complete() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::CommandExecuted {
                ts: ts(),
                step: 0,
                exit_code: 0,
                duration: 1.0,
                stdout: String::new(),
                stderr: String::new(),
            },
        ];
        let state = replay(&events, 1).unwrap();
        assert_eq!(state.status, TaskStatus::Completed);
    }

    #[test]
    fn test_reset_clears_state() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::CommandExecuted {
                ts: ts(),
                step: 0,
                exit_code: 0,
                duration: 1.0,
                stdout: String::new(),
                stderr: String::new(),
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
            Event::CommandExecuted {
                ts: ts(),
                step: 0,
                exit_code: 0,
                duration: 1.0,
                stdout: String::new(),
                stderr: String::new(),
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
    fn test_step_rolled_back() {
        let events = vec![
            Event::TaskStarted { ts: ts() },
            Event::CommandExecuted {
                ts: ts(),
                step: 0,
                exit_code: 0,
                duration: 1.0,
                stdout: String::new(),
                stderr: String::new(),
            },
            Event::StepRolledBack {
                ts: ts(),
                from_step: 1,
                to_step: 0,
            },
        ];
        let state = replay(&events, 3).unwrap();
        assert_eq!(state.current_step, 0);
        assert_eq!(state.status, TaskStatus::Waiting);
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
        let event = Event::CommandExecuted {
            ts: ts(),
            step: 0,
            exit_code: 0,
            duration: 5.2,
            stdout: "output".to_string(),
            stderr: "".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"command_executed""#));
        let _: Event = serde_json::from_str(&json).unwrap();
    }
}
