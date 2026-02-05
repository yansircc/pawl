use serde::{Deserialize, Serialize};

/// Log entry for a workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StepLog {
    /// Normal command step execution
    #[serde(rename = "command")]
    Command {
        step: usize,
        exit_code: i32,
        duration: f64,
        stdout: String,
        stderr: String,
    },

    /// Step executed in tmux window
    #[serde(rename = "in_window")]
    InWindow {
        step: usize,
        session_id: Option<String>,
        transcript: Option<String>,
        status: String,
    },

    /// Checkpoint step
    #[serde(rename = "checkpoint")]
    Checkpoint { step: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_log_serialization() {
        let log = StepLog::Command {
            step: 0,
            exit_code: 0,
            duration: 5.2,
            stdout: "output".to_string(),
            stderr: "".to_string(),
        };

        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains(r#""type":"command""#));
        assert!(json.contains(r#""step":0"#));
        assert!(json.contains(r#""exit_code":0"#));
    }

    #[test]
    fn test_in_window_log_serialization() {
        let log = StepLog::InWindow {
            step: 1,
            session_id: Some("abc123".to_string()),
            transcript: Some("/path/to/transcript.jsonl".to_string()),
            status: "success".to_string(),
        };

        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains(r#""type":"in_window""#));
        assert!(json.contains(r#""session_id":"abc123""#));
    }

    #[test]
    fn test_checkpoint_log_serialization() {
        let log = StepLog::Checkpoint { step: 2 };

        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains(r#""type":"checkpoint""#));
        assert!(json.contains(r#""step":2"#));
    }

    #[test]
    fn test_deserialization() {
        let json = r#"{"type":"command","step":0,"exit_code":0,"duration":1.5,"stdout":"ok","stderr":""}"#;
        let log: StepLog = serde_json::from_str(json).unwrap();

        match log {
            StepLog::Command {
                step,
                exit_code,
                duration,
                ..
            } => {
                assert_eq!(step, 0);
                assert_eq!(exit_code, 0);
                assert!((duration - 1.5).abs() < 0.01);
            }
            _ => panic!("Expected Command variant"),
        }
    }
}
