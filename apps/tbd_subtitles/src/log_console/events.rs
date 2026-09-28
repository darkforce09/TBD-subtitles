//! What the log window asks the application to do.

use tracing::Level;

/// One request from the log window.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LogConsoleEvent {
    /// Show the lines at this level and the ones more severe.
    Level(Level),
    /// Show only the lines holding this text; empty shows every line.
    Search(String),
    /// Forget the lines kept so far.
    Clear,
    /// Open the log file in the desktop's text editor.
    OpenLogFile,
}
