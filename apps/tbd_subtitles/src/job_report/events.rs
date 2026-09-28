//! What the Overview asks the application to do.

use std::path::PathBuf;

use crate::job_report::models::finding_group::LineGroup;

/// One request from the Overview.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ReportEvent {
    /// Open this video in the desktop's video player.
    OpenVideo(PathBuf),
    /// Show this file in the file manager, in its folder.
    ShowInFolder(PathBuf),
    /// Open this report in the desktop's text editor.
    OpenReport(PathBuf),
    /// The subtitle file's path was put on the clipboard.
    Copied,
    /// Open Check Lines on these lines.
    CheckLines(LinesToCheck),
    /// Run the job's language-model calls again, and every step after them.
    TryAgain,
}

/// The lines Check Lines opens on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LinesToCheck {
    /// The lines worth a listen.
    Flagged,
    /// The lines of one group.
    Group(LineGroup),
    /// Every line, flagged or not, at the one nearest this video second.
    Near(f64),
}
