//! The pipeline's one error type: what was being done, why it failed, and what kind of end it is.
//!
//! **Role:** carry every failure of the crate as `PipelineError`, with `ErrorKind` telling a
//! failure from a stop the owner asked for and from a job whose database another process owns.
//!
//! **Position:** returned by every fallible function of the crate; the app's window reads the kind
//! to show a job cancelled or busy instead of failed.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** a busy error's message is "process {pid} is already running it", or "another
//! process is already running it" when the owner is not known; `Context` always makes a failure.

use std::fmt;

/// What kind of end an error is: a failure, a stop the owner asked for, or a job another process
/// owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Something broke.
    Failed,
    /// The job stopped because its cancel token was set, not because something broke.
    Cancelled,
    /// Another process has the job's database open; `owner` is its pid when `job.lock` names it.
    Busy { owner: Option<u32> },
}

/// A failed step, file or process, with what was being done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineError {
    pub context: String,
    pub message: String,
    pub kind: ErrorKind,
}

impl PipelineError {
    pub fn new(context: impl Into<String>, message: impl fmt::Display) -> PipelineError {
        PipelineError {
            context: context.into(),
            message: message.to_string(),
            kind: ErrorKind::Failed,
        }
    }

    /// The job was stopped by its cancel token while doing `context`.
    pub fn cancelled(context: impl Into<String>) -> PipelineError {
        PipelineError {
            context: context.into(),
            message: "cancelled".to_string(),
            kind: ErrorKind::Cancelled,
        }
    }

    /// Another process, `owner` when known, has the database of the job in `context` open.
    pub fn busy(context: impl Into<String>, owner: Option<u32>) -> PipelineError {
        let message = match owner {
            Some(pid) => format!("process {pid} is already running it"),
            None => "another process is already running it".to_string(),
        };
        PipelineError {
            context: context.into(),
            message,
            kind: ErrorKind::Busy { owner },
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.kind == ErrorKind::Cancelled
    }

    /// `Some(owner)` when another process owns the job, with its pid when known; `None` for any
    /// other error.
    pub fn busy_owner(&self) -> Option<Option<u32>> {
        match self.kind {
            ErrorKind::Busy { owner } => Some(owner),
            ErrorKind::Failed | ErrorKind::Cancelled => None,
        }
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
