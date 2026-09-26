//! What the queue panel and the progress view ask the application to do.

use crate::job_queue::models::queue::{JobId, Move};

/// An event the queue panel returns; the application applies it after the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JobQueueEvent {
    /// Open the desktop's chooser for video files.
    AddVideos,
    /// Open the desktop's chooser for a folder of videos.
    AddFolder,
    /// Show this job on the right.
    Select(JobId),
    /// Take this job out of the queue.
    Remove(JobId),
    Move(JobId, Move),
    /// Stop this running job; its finished steps stay valid.
    Cancel(JobId),
    /// Put this ended job back to waiting.
    Retry(JobId),
    /// Run the waiting jobs, one after another.
    Start,
    /// Start no further job once the running one ends.
    Pause,
}
