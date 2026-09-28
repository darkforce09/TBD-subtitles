//! The application's actions: every change a frame asks for, applied after the frame.
//!
//! **Role:** name each change the window can make, wrap each feature's events as one, and say
//! an action in the log.
//!
//! **Position:** built by the frame (`window`, `feature_views`, `shortcuts`, the Settings and log
//! windows) and by the actions that offer a toast's button; applied by `TbdSubtitlesApp::apply`.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** an action names the job it acts on where the job shown may change before it is
//! applied (Fix It, Stop, See Changes); the log's words for an action stay within [`DESCRIBED`]
//! characters.

use std::path::PathBuf;

use super::detail_view::DetailTab;
use crate::core::toast::ToastId;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::JobId;
use crate::job_report::events::ReportEvent;
use crate::line_review::events::ReviewEvent;
use crate::log_console::events::LogConsoleEvent;
use crate::settings::events::SettingsEvent;

/// A change to the application state, collected while a frame is drawn.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Action {
    /// Add these videos, or the videos of these folders, to the end of the queue.
    QueueVideos(Vec<PathBuf>),
    Queue(JobQueueEvent),
    /// Show this tab of the selected finished job: its report, or its lines to check.
    ShowTab(DetailTab),
    /// Open the Settings window (on General, unless it is open on a tab), or close it.
    ShowSettings(bool),
    /// Open the log window, or bring it to the front when it is open; or close it.
    ShowLog(bool),
    /// The button of this toast was pressed: take the toast away and do what it offers.
    ToastButton(ToastId),
    /// Select this finished job and open Check Lines on the lines Claude changed.
    SeeFixChanges(JobId),
    /// Start Fix It on this finished job's video.
    FixIt(JobId),
    /// Stop the Fix It run of this job's video.
    StopFix(JobId),
    Settings(SettingsEvent),
    Report(ReportEvent),
    Review(ReviewEvent),
    LogConsole(LogConsoleEvent),
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

impl From<LogConsoleEvent> for Action {
    fn from(event: LogConsoleEvent) -> Action {
        Action::LogConsole(event)
    }
}

impl Action {
    /// The action as the log names it, cut to [`DESCRIBED`] characters: a settings edit's whole
    /// settings would drown the log.
    pub(crate) fn describe(&self) -> String {
        let text = format!("{self:?}");
        match text.char_indices().nth(DESCRIBED) {
            Some((cut, _)) => format!("{}…", &text[..cut]),
            None => text,
        }
    }
}

/// The most characters of an action the log shows.
const DESCRIBED: usize = 160;
