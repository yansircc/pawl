use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Task state — pure projection type reconstructed by replay()
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    /// Current step index (0-based)
    pub current_step: usize,

    /// Overall task status
    pub status: TaskStatus,

    /// When the task was started
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,

    /// When the status was last updated
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,

    /// Status of each step (by step index)
    #[serde(default)]
    pub step_status: HashMap<usize, StepStatus>,

    /// Optional message (e.g., failure reason)
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    #[default]
    Pending,
    Running,
    Waiting,
    Completed,
    Failed,
    Stopped,
}

impl TaskStatus {
    /// Whether `target` is reachable from `self` without a reset.
    pub fn can_reach(self, target: Self) -> bool {
        if self == target {
            return true;
        }
        match self {
            Self::Completed => false,
            Self::Failed | Self::Stopped => {
                // Failed/Stopped can only naturally reach other Failed/Stopped
                !matches!(target, Self::Running | Self::Waiting | Self::Completed)
            }
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Success,
    Failed,
    Skipped,
}
