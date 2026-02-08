/// Typed error for CLI exit codes.
/// Each variant maps to a specific exit code (2-7). Internal errors remain anyhow (exit 1).
#[derive(Debug)]
pub enum PawlError {
    StateConflict { task: String, status: String, message: String },
    Precondition { message: String },
    NotFound { message: String },
    AlreadyExists { message: String },
    Validation { message: String },
    Timeout { message: String },
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
            Self::Timeout { message } => write!(f, "{}", message),
        }
    }
}

impl std::error::Error for PawlError {}
