//! The application's actions: every change a frame asks for, applied after the frame.

use std::path::PathBuf;

use crate::job_queue::events::JobQueueEvent;
use crate::job_report::events::ReportEvent;
use crate::line_review::events::ReviewEvent;
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
    /// Add these videos, or the videos of these folders, to the end of the queue.
    QueueVideos(Vec<PathBuf>),
    Queue(JobQueueEvent),
    ShowPage(Page),
    Settings(SettingsEvent),
    Report(ReportEvent),
    Review(ReviewEvent),
}

impl From<JobQueueEvent> for Action {
    fn from(event: JobQueueEvent) -> Action {
        Action::Queue(event)
    }
}

impl From<SettingsEvent> for Action {
    fn from(event: SettingsEvent) -> Action {
        Action::Settings(event)
    }
}

impl From<ReportEvent> for Action {
    fn from(event: ReportEvent) -> Action {
        Action::Report(event)
    }
}

impl From<ReviewEvent> for Action {
    fn from(event: ReviewEvent) -> Action {
        Action::Review(event)
    }
}
