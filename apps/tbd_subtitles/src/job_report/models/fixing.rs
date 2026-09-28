//! What the file card shows about Fix It: nothing, the button, the button off with why, the run
//! under way with its step (or waiting for a free `claude` call), or the correction run that puts
//! its changes into the subtitles.
//!
//! **Role:** the Overview's `FixView`, and the words of its steps and of the line under the note
//! while a run goes.
//!
//! **Position:** built by the application's Fix It actions; drawn by `job_report::ui::file_card`;
//! the steps are also counted by the sidebar's status line.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** the window counts a Fix It run in four steps: its three passes (reading,
//! fixing, checking and saving), then the correction run that times the changed lines and
//! rewrites the file; Stop outranks the wait for a free `claude` call in the line under the note.

use pipeline::fix_it::FixStage;

use crate::core::format::plural;

/// The steps of a Fix It run as the window counts them.
pub(crate) const FIX_STEPS: usize = 4;
/// The step of the correction run that puts the changes into the subtitles.
pub(crate) const UPDATING_STEP: usize = 4;

/// Fix It on the Overview of one job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FixView {
    /// Nothing Fix It would ask about.
    Hidden,
    /// Fix It can run: `model` as the window names it, such as "Claude Opus".
    Ready { model: String },
    /// Fix It cannot run now, for `reason`.
    Unavailable { model: String, reason: String },
    /// Fix It runs on this video: its stage and the calls of it done.
    Running {
        model: String,
        stage: FixStage,
        done: usize,
        total: usize,
        /// Stop was pressed and the `claude` processes are ending.
        stopping: bool,
        /// Every call of the run waits for a free slot under the cap on `claude` calls at once,
        /// which other runs hold.
        waiting: bool,
    },
    /// Fix It changed lines, and the correction run that puts them into the subtitles waits or
    /// runs.
    Updating { model: String },
}

impl FixView {
    /// Whether Fix It is under way on the video: running, or its correction run pending.
    pub(crate) fn under_way(&self) -> bool {
        matches!(self, FixView::Running { .. } | FixView::Updating { .. })
    }
}

/// The step of a run in `stage`, from 1.
pub(crate) fn step_of(stage: FixStage) -> usize {
    stage.pass()
}

/// What Fix It is doing in `stage`, as the note under way says it, with its step of four.
pub(crate) fn stage_words(stage: FixStage) -> String {
    let doing = match stage {
        FixStage::Reading => "reading the whole video",
        FixStage::Fixing(family) => match family {
            job_model::outputs::FixFamily::Words => "fixing words",
            job_model::outputs::FixFamily::Timing => "fixing timing and layout",
            job_model::outputs::FixFamily::ReadingSpeed => "fixing reading speed",
        },
        FixStage::Checking => "checking each change",
        FixStage::Saving => "saving the changes",
    };
    format!("{doing} ({} of {FIX_STEPS})", step_of(stage))
}

/// The line under the note while Fix It runs: Stop pressed, waiting for a free `claude` call, or
/// `done` of `total` calls done.
pub(crate) fn running_line(done: usize, total: usize, stopping: bool, waiting: bool) -> String {
    let calls = format!("{done} of {} done.", plural(total, "call"));
    if stopping {
        "Stopping. Nothing is changed; Fix It again picks up where it stopped.".to_string()
    } else if waiting {
        format!("Waiting for a free Claude call. {calls}")
    } else {
        format!("{calls} The subtitles change only once every change is checked.")
    }
}

/// What Fix It is doing while its correction run waits or runs, with its step of four.
pub(crate) fn updating_words() -> String {
    format!("updating the subtitles ({UPDATING_STEP} of {FIX_STEPS})")
}

#[cfg(test)]
#[path = "tests/fixing.rs"]
mod tests;
