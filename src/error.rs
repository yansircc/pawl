use serde::Serialize;

/// Structured error type for agent-consumable error output.
/// Each variant maps to a specific exit code and JSON stderr output.
#[derive(Debug, Serialize)]
#[serde(tag = "error", content = "detail")]
pub enum PawlError {
    /// Task is in a state that conflicts with the requested operation (exit 2)
    StateConflict { task: String, status: String, message: String },
    /// A precondition for the operation is not met (exit 3)
    Precondition { message: String },
    /// Referenced resource does not exist (exit 4)
    NotFound { message: String },
    /// Resource already exists (exit 5)
    AlreadyExists { message: String },
    /// Input validation failed (exit 6)
    Validation { message: String },
    /// Operation timed out (exit 7)
    Timeout { task: String, message: String },
}

impl PawlError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::StateConflict { .. } => 2,
            Self::Precondition { .. } => 3,
            Self::NotFound { .. } => 4,
            Self::AlreadyExists { .. } => 5,
            Self::Validation { .. } => 6,
            Self::Timeout { .. } => 7,
        }
    }

    pub fn suggest(&self) -> Vec<String> {
        match self {
            Self::StateConflict { task, status, .. } => match status.as_str() {
                "running" => vec![
                    format!("pawl stop {task}"),
                    format!("pawl start --reset {task}"),
                ],
                "completed" => vec![
                    format!("pawl reset {task}"),
                    format!("pawl start --reset {task}"),
                ],
                "waiting" => vec![format!("pawl stop {task}")],
                "stopped" => vec![
                    format!("pawl start {task}"),
                    format!("pawl reset {task}"),
                ],
                "pending" => vec![format!("pawl start {task}")],
                _ => vec![],
            },
            Self::NotFound { message } if message.contains("pawl init") => {
                vec!["pawl init".to_string()]
            }
            Self::Timeout { task, .. } => vec![format!("pawl status {task}")],
            _ => vec![],
        }
    }
}

impl std::fmt::Display for PawlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StateConflict { task, status, message } => {
                write!(f, "Task '{}' is {} — {}", task, status, message)
            }
            Self::Precondition { message } => write!(f, "{}", message),
            Self::NotFound { message } => write!(f, "{}", message),
            Self::AlreadyExists { message } => write!(f, "{}", message),
            Self::Validation { message } => write!(f, "{}", message),
            Self::Timeout { message, .. } => write!(f, "{}", message),
        }
    }
}

impl std::error::Error for PawlError {}
