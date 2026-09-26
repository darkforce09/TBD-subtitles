//! The application's actions: every change a frame asks for, applied after the frame.

use std::path::PathBuf;

use crate::job_queue::events::JobQueueEvent;
use crate::settings::events::SettingsEvent;

/// The page the right side of the window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    /// The selected job: its progress, report or review.
    Jobs,
    Settings,
}

/// A change to the application state, collected while a frame is drawn.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Action {
    /// Add these videos to the end of the queue, skipping those already queued.
    QueueVideos(Vec<PathBuf>),
    /// Take the video at this queue position out of the queue.
    RemoveFromQueue(usize),
    /// Open the desktop's chooser to add videos, or a folder of videos.
    ChooseForQueue {
        folder: bool,
    },
    ShowPage(Page),
    Settings(SettingsEvent),
}

impl From<JobQueueEvent> for Action {
    fn from(event: JobQueueEvent) -> Action {
        match event {
            JobQueueEvent::Remove(index) => Action::RemoveFromQueue(index),
            JobQueueEvent::AddVideos => Action::ChooseForQueue { folder: false },
            JobQueueEvent::AddFolder => Action::ChooseForQueue { folder: true },
        }
    }
}

impl From<SettingsEvent> for Action {
    fn from(event: SettingsEvent) -> Action {
        Action::Settings(event)
    }
}
