pub mod tmux;

use anyhow::Result;
use crate::error::PawlError;

/// A viewport is an execution surface for in_viewport steps.
/// It provides the ability to open, execute commands in, check existence of,
/// and close named execution contexts.
pub trait Viewport {
    fn open(&self, name: &str, cwd: &str) -> Result<()>;
    fn execute(&self, name: &str, text: &str) -> Result<()>;
    fn exists(&self, name: &str) -> bool;
    fn close(&self, name: &str) -> Result<()>;
}

/// Create a viewport instance based on the backend name.
pub fn create_viewport(backend: &str, session: &str) -> Result<Box<dyn Viewport>> {
    match backend {
        "tmux" => Ok(Box::new(tmux::TmuxViewport::new(session))),
        other => Err(PawlError::Validation {
            message: format!("Unsupported viewport backend: {}", other),
        }.into()),
    }
}
