//! The application's actions: every change a frame asks for, applied after the frame.

use std::path::PathBuf;

use crate::core::toast::ToastId;
use crate::job_queue::events::JobQueueEvent;
use crate::job_report::events::ReportEvent;
use crate::line_review::events::ReviewEvent;
use crate::settings::events::SettingsEvent;

/// A change to the application state, collected while a frame is drawn.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Action {
    /// Add these videos, or the videos of these folders, to the end of the queue.
    QueueVideos(Vec<PathBuf>),
    Queue(JobQueueEvent),
    /// Open the Settings window, or close it.
    ShowSettings(bool),
    /// The button of this toast was pressed: take the toast away and do what it offers.
    ToastButton(ToastId),
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
