pub mod tmux;

use anyhow::Result;
use crate::error::PawlError;

/// A viewport is an observable execution surface.
/// It provides the ability to open, execute commands in, read from, check existence of,
/// close, and attach to named execution contexts.
pub trait Viewport {
    fn open(&self, name: &str, cwd: &str) -> Result<()>;
    fn execute(&self, name: &str, text: &str) -> Result<()>;
    fn read(&self, name: &str, lines: usize) -> Result<Option<String>>;
    fn exists(&self, name: &str) -> bool;
    fn is_active(&self, name: &str) -> bool;
    fn close(&self, name: &str) -> Result<()>;
    fn attach(&self, name: &str) -> Result<()>;
}

/// Create a viewport instance based on the backend name.
pub fn create_viewport(backend: &str, session: &str) -> Result<Box<dyn Viewport>> {
    match backend {
        "tmux" => Ok(Box::new(tmux::TmuxViewport::new(session))),
        other => return Err(PawlError::Validation {
            message: format!("Unsupported viewport backend: {}", other),
        }.into()),
    }
}
