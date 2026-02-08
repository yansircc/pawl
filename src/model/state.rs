use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::PawlError;

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

    /// Run ID (UUID v4) for this execution instance
    pub run_id: String,
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

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Waiting => write!(f, "waiting"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Stopped => write!(f, "stopped"),
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "waiting" => Ok(Self::Waiting),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "stopped" => Ok(Self::Stopped),
            _ => return Err(PawlError::Validation {
                message: format!("Invalid status '{}'. Valid values: pending, running, waiting, completed, failed, stopped", s),
            }.into()),
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

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "success"),
            Self::Failed => write!(f, "failed"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}
