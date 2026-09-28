//! The groups the owner sees the quality check's line findings in, each with its title and a
//! plain explanation.
//!
//! **Role:** fold the checks about lines into six groups the owner recognises: unsure lines,
//! heard words replaced, words no engine heard, lines too fast to read, loosely timed lines and
//! layout; and a seventh, first, for the lines Fix It changed that the owner has not checked.
//!
//! **Position:** read by `services::line_counts` to count lines per group, by the lines card for
//! each group's row, and by the line review for a line's chips and its group filter.
//!
//! **Signals and state:** none; constants only.
//!
//! **Invariants:** every check about a line has exactly one group, never the Fix It group, which
//! comes from the corrections; the checks about the whole job (heard speech with no cue, the
//! aligner's offset, a failed language-model call) have none, and are problems instead.

use job_model::report::QcCheck;

/// A group of line findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum LineGroup {
    /// Changed by Fix It, and not kept or undone by the owner yet.
    ChangedByFixIt,
    /// The language model could not settle what was said.
    Unsure,
    /// A word both engines heard that the subtitles do not use (`removed_locked`).
    HeardWordReplaced,
    /// A word no engine heard (`novel`).
    NovelWord,
    /// Over 20 characters per second.
    TooFast,
    /// Fewer than half the words timed by the aligner.
    LooselyTimed,
    /// A subtitle breaking a layout rule.
    Layout,
}

impl LineGroup {
    /// Every group, in the order the lines card lists them.
    pub(crate) const ALL: [LineGroup; 7] = [
        LineGroup::ChangedByFixIt,
        LineGroup::Unsure,
        LineGroup::HeardWordReplaced,
        LineGroup::NovelWord,
        LineGroup::TooFast,
        LineGroup::LooselyTimed,
        LineGroup::Layout,
    ];

    /// The group of `check`; none for a check about the whole job.
    pub(crate) fn of(check: QcCheck) -> Option<LineGroup> {
        match check {
            QcCheck::Unsure => Some(LineGroup::Unsure),
            QcCheck::RemovedLocked => Some(LineGroup::HeardWordReplaced),
            QcCheck::Novel => Some(LineGroup::NovelWord),
            QcCheck::TooFast => Some(LineGroup::TooFast),
            QcCheck::WeakTiming => Some(LineGroup::LooselyTimed),
            QcCheck::Overlap
            | QcCheck::GapTooSmall
            | QcCheck::TooShort
            | QcCheck::TooLong
            | QcCheck::LineTooLong
            | QcCheck::TooManyLines
            | QcCheck::Empty
            | QcCheck::PastEnd => Some(LineGroup::Layout),
            QcCheck::UncoveredSpeech | QcCheck::Offset | QcCheck::FailedCall => None,
        }
    }

    /// The group's title on its row.
    pub(crate) fn title(self) -> &'static str {
        match self {
            LineGroup::ChangedByFixIt => "Changed by Claude",
            LineGroup::Unsure => "Unsure what was said",
            LineGroup::HeardWordReplaced => "Heard word replaced",
            LineGroup::NovelWord => "Word no engine heard",
            LineGroup::TooFast => "Too fast to read",
            LineGroup::LooselyTimed => "Loosely timed",
            LineGroup::Layout => "Layout",
        }
    }

    /// The group's short name on a line's chip in the Check Lines list.
    pub(crate) fn chip(self) -> &'static str {
        match self {
            LineGroup::ChangedByFixIt => "Claude",
            LineGroup::Unsure => "Unsure",
            LineGroup::HeardWordReplaced => "Word replaced",
            LineGroup::NovelWord => "Word no engine heard",
            LineGroup::TooFast => "Too fast",
            LineGroup::LooselyTimed => "Loosely timed",
            LineGroup::Layout => "Layout",
        }
    }

    /// What the group means, in plain words, under its title.
    pub(crate) fn explanation(self) -> &'static str {
        match self {
            LineGroup::ChangedByFixIt => {
                "Fix It changed these lines, and its last check accepted each change. Listen, \
                 then keep it or undo it."
            }
            LineGroup::Unsure => {
                "The two speech engines disagreed, and a second listen to the voices alone \
                 didn't settle it. The app's best guess is in the file."
            }
            LineGroup::HeardWordReplaced => {
                "Both engines heard a word the subtitles don't use. Here that is nearly always a \
                 name spelled to match the glossary (Frankie → Franky)."
            }
            LineGroup::NovelWord => {
                "A word neither engine heard, usually a name the language model took from the \
                 glossary."
            }
            LineGroup::TooFast => {
                "More than 20 characters per second. Optional: a shorter wording reads more \
                 easily."
            }
            LineGroup::LooselyTimed => {
                "Fewer than half the words were timed by the aligner. Looks Right re-times the \
                 line."
            }
            LineGroup::Layout => {
                "A subtitle breaks a layout rule. Editing the line rebuilds its subtitles."
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/finding_group.rs"]
mod tests;
