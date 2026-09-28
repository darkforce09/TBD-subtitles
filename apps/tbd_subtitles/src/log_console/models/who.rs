//! Who wrote a log line: the app, a job, the AI, or a program the app started.
//!
//! **Role:** name the four kinds of writer the log window tells apart, and find a line's from its
//! target.
//!
//! **Position:** used by the activity list for its chips and its filter.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** every target is exactly one kind; the model's own lines and Fix It's are the
//! AI's, whichever process wrote them.

/// Who wrote a log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Who {
    /// The window itself: what the owner did, the desktop, the settings.
    App,
    /// A job's steps, as the pipeline runs them.
    Job,
    /// A language model: its calls and Fix It's passes.
    Ai,
    /// A program the app started: FFmpeg, ffprobe, `claude`, a worker.
    Program,
}

impl Who {
    /// The four, as the filter lists them.
    pub(crate) const ALL: [Who; 4] = [Who::App, Who::Job, Who::Ai, Who::Program];

    /// Who writes under `target`.
    pub(crate) fn of(target: &str) -> Who {
        let under = |root: &str| target == root || target.starts_with(&format!("{root}::"));
        if under("inference::llm") || under("fix_it") || under("pipeline::fix_it") {
            Who::Ai
        } else if under("child_process") {
            Who::Program
        } else if under("job")
            || under("pipeline")
            || under("stages")
            || under("media_io")
            || under("subtitle_formats")
        {
            Who::Job
        } else {
            Who::App
        }
    }

    /// The chip's word.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Who::App => "App",
            Who::Job => "Job",
            Who::Ai => "AI",
            Who::Program => "Program",
        }
    }

    /// The filter's word, for many lines.
    pub(crate) fn plural(self) -> &'static str {
        match self {
            Who::App => "App",
            Who::Job => "Jobs",
            Who::Ai => "AI",
            Who::Program => "Programs",
        }
    }
}

#[cfg(test)]
#[path = "tests/who.rs"]
mod tests;
