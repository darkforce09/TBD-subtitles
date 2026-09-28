//! What the log window asks the application to do.

use tracing::Level;

use crate::log_console::models::console::ConsoleView;
use crate::log_console::models::who::Who;

/// One request from the log window.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LogConsoleEvent {
    /// Show this view: every line, or the model calls.
    View(ConsoleView),
    /// Show the lines at this level and the ones more severe.
    Level(Level),
    /// Show only the lines this writer wrote, or every line.
    Who(Option<Who>),
    /// Show only the lines and calls holding this text; empty shows everything.
    Search(String),
    /// Open this line in the detail panel, or close the panel.
    SelectLine(Option<u64>),
    /// Open this call on the right of the Model Calls view, or none.
    SelectCall(Option<String>),
    /// Open the Model Calls view on this call.
    ShowCall(String),
    /// Forget the lines, or the calls, of the view shown.
    Clear,
    /// Open the log file in the desktop's text editor.
    OpenLogFile,
}
