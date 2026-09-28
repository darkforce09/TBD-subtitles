//! What the file card shows about Fix It: nothing, the button, the button off with why, the run
//! under way with its step, or the correction run that puts its changes into the subtitles.
//!
//! The window counts a Fix It run in four steps: its three passes (reading, fixing, checking and
//! saving), then the correction run that times the changed lines and rewrites the file.

use pipeline::fix_it::FixStage;

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

/// What Fix It is doing while its correction run waits or runs, with its step of four.
pub(crate) fn updating_words() -> String {
    format!("updating the subtitles ({UPDATING_STEP} of {FIX_STEPS})")
}

#[cfg(test)]
#[path = "tests/fixing.rs"]
mod tests;
