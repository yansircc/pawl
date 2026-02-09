use anyhow::Result;

use crate::error::PawlError;

/// Get the project root by walking up from cwd looking for `.pawl/` directory
pub fn get_project_root() -> Result<String> {
    let mut dir = std::env::current_dir()
        .map_err(|e| anyhow::anyhow!("Failed to get current directory: {}", e))?;

    loop {
        if dir.join(".pawl").is_dir() {
            return Ok(dir.to_string_lossy().to_string());
        }
        if !dir.pop() {
            return Err(PawlError::NotFound {
                message: "Not a pawl project (no .pawl/ found). Run 'pawl init' first.".into(),
            }
            .into());
        }
    }
}
