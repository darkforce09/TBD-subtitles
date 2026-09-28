//! What the toolbar, the sidebar and the progress view ask the application to do.

use std::path::PathBuf;

use job_model::StepName;

use crate::job_queue::models::queue::{JobId, Move};

/// An event the queue's views return; the application applies it after the frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JobQueueEvent {
    /// Open the desktop's chooser for video files.
    AddVideos,
    /// Open the desktop's chooser for a folder of videos.
    AddFolder,
    /// Show this job on the right.
    Select(JobId),
    /// Take this row out of the list; its files stay on disk.
    Remove(JobId),
    /// Put the row of this job back where it was, if it is the row removed last.
    Undo(JobId),
    Move(JobId, Move),
    /// Move this waiting job to just before that one, or last when none is named.
    MoveBefore(JobId, Option<JobId>),
    /// Stop this running job; its finished steps stay valid.
    Cancel(JobId),
    /// Run this failed or cancelled job again first, from the steps it kept, or from this step.
    TryAgain(JobId, Option<StepName>),
    /// Run this finished job again, first, with the settings saved now.
    RunAgain(JobId),
    /// Open this finished job's lines to check.
    CheckLines(JobId),
    /// Show this file in the file manager.
    Reveal(PathBuf),
    /// Open this video in the desktop's video player.
    OpenVideo(PathBuf),
    /// A subtitle path was copied to the clipboard.
    Copied,
    /// Run the waiting jobs, one after another.
    Start,
    /// Start no further job once the running one ends.
    Pause,
}
