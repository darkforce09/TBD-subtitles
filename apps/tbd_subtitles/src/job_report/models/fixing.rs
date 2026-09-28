//! What the file card shows about Fix It: nothing, the button, the button off with why, or the
//! run under way with its pass.

use pipeline::fix_it::FixStage;

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
}

/// What Fix It is doing in `stage`, as the note under way says it, with its pass of three.
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
    format!("{doing} ({} of 3)", stage.pass())
}

#[cfg(test)]
#[path = "tests/fixing.rs"]
mod tests;
