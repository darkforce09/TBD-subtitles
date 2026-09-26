//! What the report view asks the application to do.

use std::path::PathBuf;

/// One request from the report view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReportEvent {
    /// Ask the desktop to open this file or folder: the video in its default player, the report
    /// in its text editor, a folder in the file manager.
    Open(PathBuf),
    /// Open the job's line review, at this utterance when one is named.
    Review(Option<String>),
}
