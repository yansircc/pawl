use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDefinition {
    /// Task name (derived from filename)
    pub name: String,

    /// Dependencies (other task names)
    #[serde(default)]
    pub depends: Vec<String>,

    /// Task description (markdown body)
    #[serde(default)]
    pub description: String,
}
