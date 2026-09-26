//! A review of one job's lines: every utterance with what each engine heard, what the language
//! model settled on, why it was flagged, and the owner's correction, plus the line being edited.
//!
//! **Role:** hold the lines of one finished job and the owner's edit in progress.
//!
//! **Position:** built by `line_review::services::review_loading`; changed by
//! `review_editing`; drawn by `line_review::ui`.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** lines are in sheet order; a line is flagged when a finding names it or the
//! language model left it unsure.

use std::path::PathBuf;

use job_model::outputs::{Correction, Corrections};

/// One engine's reading of an utterance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hypothesis {
    /// `P` (Parakeet), `W` (Whisper), `ALT p` and `ALT w` (heard again on the vocal stem), or
    /// `adjudicated` (the language model's text).
    pub(crate) tag: String,
    pub(crate) text: String,
}

/// One utterance.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReviewLine {
    pub(crate) id: String,
    pub(crate) start_s: f64,
    pub(crate) end_s: f64,
    /// The language model's text (`||` starts another speaker).
    pub(crate) adjudicated: String,
    /// The language model's flags.
    pub(crate) flags: Vec<String>,
    pub(crate) hypotheses: Vec<Hypothesis>,
    /// Why the line is flagged: the quality check's findings about it.
    pub(crate) reasons: Vec<String>,
}

impl ReviewLine {
    pub(crate) fn flagged(&self) -> bool {
        !self.reasons.is_empty() || self.flags.iter().any(|f| f == "UNSURE")
    }
}

/// The line being edited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Draft {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) flags: Vec<String>,
}

/// The review of one job.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReviewSession {
    pub(crate) video: PathBuf,
    pub(crate) work_dir: PathBuf,
    /// The job's audio track, for playing clips.
    pub(crate) audio_position: u32,
    /// The video's picture size, for the preview frames.
    pub(crate) picture: (u32, u32),
    pub(crate) lines: Vec<ReviewLine>,
    pub(crate) corrections: Corrections,
    /// Show every line, not only the flagged ones.
    pub(crate) show_all: bool,
    pub(crate) draft: Option<Draft>,
    /// What the last save did, or why it failed.
    pub(crate) notice: Option<String>,
}

impl ReviewSession {
    /// The lines the list shows.
    pub(crate) fn shown(&self) -> impl Iterator<Item = &ReviewLine> {
        self.lines.iter().filter(|line| {
            self.show_all || line.flagged() || self.corrections.get(&line.id).is_some()
        })
    }

    pub(crate) fn line(&self, id: &str) -> Option<&ReviewLine> {
        self.lines.iter().find(|line| line.id == id)
    }

    pub(crate) fn correction(&self, id: &str) -> Option<&Correction> {
        self.corrections.get(id)
    }
}
