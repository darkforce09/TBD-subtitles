//! A review of one job's lines: every utterance with what each engine heard, what the language
//! model settled on, why the quality check flagged it, and the owner's corrections, plus the
//! line open in the editor, the owner's unsaved edits, the list's filter, and where each saved
//! line's correction run stands.
//!
//! **Role:** hold the lines of one finished job and everything the owner has done to them in
//! this window.
//!
//! **Position:** built by `line_review::services::review_loading`; changed by `review_editing`;
//! read by `line_filter` and drawn by `line_review::ui`.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** lines are in sheet order; a line is flagged exactly when the quality check put
//! it in some group, and worth a listen while flagged, corrected, or taken back with its run not
//! ended; a draft is kept only while it differs from the line's saved text or flags; each line has
//! at most one run state, the newest last.

use std::collections::BTreeMap;
use std::path::PathBuf;

use job_model::outputs::{Chosen, Correction, Corrections};

use crate::job_report::models::finding_group::LineGroup;

/// One engine's reading of an utterance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hypothesis {
    /// `P` (Parakeet), `W` (Whisper), `ALT p` and `ALT w` (heard again on the vocal stem).
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
    /// Each group the quality check put the line in, once and in the groups' order, with why in
    /// plain words from its first finding there.
    pub(crate) groups: Vec<(LineGroup, String)>,
}

impl ReviewLine {
    /// Whether the line is worth a listen: the quality check put it in a group.
    pub(crate) fn flagged(&self) -> bool {
        !self.groups.is_empty()
    }

    pub(crate) fn in_group(&self, group: LineGroup) -> bool {
        self.groups.iter().any(|(g, _)| *g == group)
    }

    /// The reading tagged `tag`: an engine's, or the language model's for `adjudicated`.
    pub(crate) fn reading(&self, tag: &str) -> Option<&str> {
        if tag == "adjudicated" {
            return Some(self.adjudicated.as_str());
        }
        self.hypotheses
            .iter()
            .find(|h| h.tag == tag)
            .map(|h| h.text.as_str())
    }

    /// The seconds the line lasts.
    pub(crate) fn length_s(&self) -> f64 {
        (self.end_s - self.start_s).max(0.0)
    }
}

/// A line's text and flags: saved, or as the owner is editing them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Draft {
    pub(crate) text: String,
    pub(crate) flags: Vec<String>,
}

/// Which lines the list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum LineList {
    /// The flagged lines not corrected yet.
    #[default]
    ToCheck,
    /// The lines the owner kept or corrected.
    Checked,
    /// Every line.
    All,
}

/// Where a saved line's correction run stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunState {
    /// Saved; its run has not started.
    Saved,
    /// Its run is timing it again and writing the subtitles.
    Updating,
    /// Its run ended and the subtitle file holds it.
    Updated,
    /// Its run failed.
    Failed,
}

/// What a line's row says about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineStatus {
    /// Edited and not saved.
    Edited,
    /// Saved as the language model had it: Looks Right.
    Kept,
    /// Saved with another text or other flags.
    Corrected,
    /// Worth a listen and not checked.
    ToCheck,
    /// Neither flagged nor checked.
    Plain,
}

/// The review of one job.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReviewSession {
    pub(crate) video: PathBuf,
    pub(crate) work_dir: PathBuf,
    /// The job's audio track, for playing clips.
    pub(crate) audio_position: u32,
    /// The video's picture size, for the preview frames; `(0, 0)` when it has none or it is not
    /// known.
    pub(crate) picture: (u32, u32),
    pub(crate) lines: Vec<ReviewLine>,
    pub(crate) corrections: Corrections,
    /// The line open in the editor, when the list shows it.
    pub(crate) open: Option<String>,
    /// The owner's unsaved edits, by line.
    pub(crate) drafts: BTreeMap<String, Draft>,
    pub(crate) list: LineList,
    /// The search field's text: words of a line, its id, or a time.
    pub(crate) search: String,
    /// The group the list is narrowed to.
    pub(crate) group: Option<LineGroup>,
    /// Where each saved line's correction run stands, the newest last.
    pub(crate) runs: Vec<(String, RunState)>,
}

impl ReviewSession {
    /// A review of `lines` with nothing open, edited or filtered.
    pub(crate) fn new(
        video: PathBuf,
        work_dir: PathBuf,
        lines: Vec<ReviewLine>,
        corrections: Corrections,
    ) -> ReviewSession {
        ReviewSession {
            video,
            work_dir,
            audio_position: 0,
            picture: (0, 0),
            lines,
            corrections,
            open: None,
            drafts: BTreeMap::new(),
            list: LineList::default(),
            search: String::new(),
            group: None,
            runs: Vec::new(),
        }
    }

    pub(crate) fn line(&self, id: &str) -> Option<&ReviewLine> {
        self.lines.iter().find(|line| line.id == id)
    }

    pub(crate) fn correction(&self, id: &str) -> Option<&Correction> {
        self.corrections.get(id)
    }

    /// The text and flags `line` has saved: its correction, else the language model's text and
    /// flags without `UNSURE`.
    pub(crate) fn saved(&self, line: &ReviewLine) -> Draft {
        match self.correction(&line.id) {
            Some(c) => Draft {
                text: c.text.clone(),
                flags: c.flags.clone(),
            },
            None => Draft {
                text: line.adjudicated.clone(),
                flags: without_unsure(&line.flags),
            },
        }
    }

    /// The text and flags `line` shows now: the owner's draft, else what it has saved.
    pub(crate) fn current(&self, line: &ReviewLine) -> Draft {
        self.drafts
            .get(&line.id)
            .cloned()
            .unwrap_or_else(|| self.saved(line))
    }

    /// Whether `line` is worth a listen: the quality check put it in a group, the owner corrected
    /// it, or the owner took its correction back and its run has not ended. A correction run drops
    /// the findings of the lines it settles, so the set holds across the run.
    pub(crate) fn worth(&self, line: &ReviewLine) -> bool {
        let taken_back = self.correction(&line.id).is_none()
            && matches!(
                self.run(&line.id),
                Some(RunState::Saved | RunState::Updating)
            );
        line.flagged() || self.correction(&line.id).is_some() || taken_back
    }

    /// Whether `line`'s correction keeps the language model's text and flags: Looks Right.
    pub(crate) fn kept(&self, line: &ReviewLine) -> bool {
        self.correction(&line.id).is_some_and(|c| {
            c.chosen == Chosen::Engine("adjudicated".to_string())
                && c.text
                    == line
                        .adjudicated
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                && same_flags(&c.flags, &without_unsure(&line.flags))
        })
    }

    /// The run state of line `id`.
    pub(crate) fn run(&self, id: &str) -> Option<RunState> {
        self.runs
            .iter()
            .find(|(line, _)| line == id)
            .map(|(_, state)| *state)
    }

    /// The run state the editor's header shows: the open line's, else the newest.
    pub(crate) fn run_shown(&self) -> Option<RunState> {
        self.open
            .as_deref()
            .and_then(|id| self.run(id))
            .or_else(|| self.runs.last().map(|(_, state)| *state))
    }
}

/// What a closed review keeps until it opens again: the owner's unsaved edits and where each
/// saved line's run stands.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Parked {
    pub(crate) drafts: BTreeMap<String, Draft>,
    pub(crate) runs: Vec<(String, RunState)>,
}

/// `flags` without `UNSURE`, which a correction never carries.
pub(crate) fn without_unsure(flags: &[String]) -> Vec<String> {
    flags
        .iter()
        .filter(|f| f.as_str() != "UNSURE")
        .cloned()
        .collect()
}

/// Whether `a` and `b` hold the same flags in any order.
pub(crate) fn same_flags(a: &[String], b: &[String]) -> bool {
    let sorted = |flags: &[String]| {
        let mut flags = flags.to_vec();
        flags.sort();
        flags
    };
    sorted(a) == sorted(b)
}
