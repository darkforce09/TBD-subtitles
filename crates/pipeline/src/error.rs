//! The pipeline's one error type: what was being done, and why it failed.

use std::fmt;

/// A failed step, file or process, with what was being done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineError {
    pub context: String,
    pub message: String,
    /// The job stopped because its cancel token was set, not because something broke.
    pub cancelled: bool,
}

impl PipelineError {
    pub fn new(context: impl Into<String>, message: impl fmt::Display) -> PipelineError {
        PipelineError {
            context: context.into(),
            message: message.to_string(),
            cancelled: false,
        }
    }

    /// The job was stopped by its cancel token while doing `context`.
    pub fn cancelled(context: impl Into<String>) -> PipelineError {
        PipelineError {
            context: context.into(),
            message: "cancelled".to_string(),
            cancelled: true,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.context, self.message)
    }
}

impl std::error::Error for PipelineError {}

pub type Result<T> = std::result::Result<T, PipelineError>;

/// Attach what was being done to any displayable error.
pub trait Context<T> {
    fn context(self, context: impl Into<String>) -> Result<T>;
}

impl<T, E: fmt::Display> Context<T> for std::result::Result<T, E> {
    fn context(self, context: impl Into<String>) -> Result<T> {
        self.map_err(|e| PipelineError::new(context, e))
    }
}
