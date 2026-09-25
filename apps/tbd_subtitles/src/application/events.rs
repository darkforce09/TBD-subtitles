//! The application's actions: every change a frame asks for, applied after the frame.

use std::path::PathBuf;

use crate::job_queue::events::JobQueueEvent;

/// A change to the application state, collected while a frame is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    /// Add these videos to the end of the queue, skipping those already queued.
    QueueVideos(Vec<PathBuf>),
    /// Take the video at this queue position out of the queue.
    RemoveFromQueue(usize),
}

impl From<JobQueueEvent> for Action {
    fn from(event: JobQueueEvent) -> Action {
        match event {
            JobQueueEvent::Remove(index) => Action::RemoveFromQueue(index),
        }
    }
}
