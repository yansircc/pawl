pub mod tmux;

use std::any::Any;

use anyhow::{bail, Result};

/// A viewport is a human-observable execution surface.
/// It provides the ability to open, send commands to, read from, check existence of,
/// close, and attach to named execution contexts.
pub trait Viewport {
    fn as_any(&self) -> &dyn Any;
    fn open(&self, name: &str, cwd: &str) -> Result<()>;
    fn send(&self, name: &str, text: &str) -> Result<()>;
    fn read(&self, name: &str, lines: usize) -> Result<Option<String>>;
    fn exists(&self, name: &str) -> bool;
    fn close(&self, name: &str) -> Result<()>;
    fn attach(&self, name: &str) -> Result<()>;
}

/// Create a viewport instance based on the backend name.
pub fn create_viewport(backend: &str, session: &str) -> Result<Box<dyn Viewport>> {
    match backend {
        "tmux" => Ok(Box::new(tmux::TmuxViewport::new(session))),
        other => bail!("Unsupported viewport backend: {}", other),
    }
}
