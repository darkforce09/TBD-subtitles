//! The quality check's result, as kept in `qc.json` and rendered into `report.md`.
//!
//! **Role:** name every check, the finding it leaves (where, what, why), and the counts that sum a
//! job up.
//!
//! **Position:** written by the QC step (`crates/stages/src/qc/`), read by the report renderer and
//! the app.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** the check names stay stable in JSON; a layout violation is one of the rules the
//! success criteria forbid outright, the other findings are for review.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The share of cues that must read at or under 20 characters per second.
pub const CPS_TARGET: f64 = 0.95;

/// One kind of problem the quality check looks for.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[rkyv(compare(PartialEq), derive(Debug, PartialEq, Eq, PartialOrd, Ord))]
pub enum QcCheck {
    /// Two cues on screen at once.
    Overlap,
    /// Fewer than 2 frames between two cues.
    GapTooSmall,
    /// Shorter than 5/6 s (20 frames at 24 fps).
    TooShort,
    /// Longer than 7 s.
    TooLong,
    /// Over 20 characters per second.
    TooFast,
    /// A line over 42 characters.
    LineTooLong,
    /// More than two lines.
    TooManyLines,
    /// A cue with no text.
    Empty,
    /// A cue ending after the video.
    PastEnd,
    /// Speech over 1 s long that an engine heard words in, outside songs, with no cue on it.
    UncoveredSpeech,
    /// The language model could not settle an utterance.
    Unsure,
    /// A word no engine heard.
    Novel,
    /// A word every engine agreed on that the language model dropped.
    RemovedLocked,
    /// An utterance timed only from the backbone or interpolated.
    WeakTiming,
    /// The aligner's median offset from the backbone is 30 ms or more.
    Offset,
    /// A language-model call failed.
    FailedCall,
}

impl QcCheck {
    /// What the check means, for the report.
    pub fn describe(self) -> &'static str {
        match self {
            QcCheck::Overlap => "cues overlap",
            QcCheck::GapTooSmall => "gap under 2 frames",
            QcCheck::TooShort => "under 20 frames",
            QcCheck::TooLong => "over 7 s",
            QcCheck::TooFast => "over 20 characters per second",
            QcCheck::LineTooLong => "line over 42 characters",
            QcCheck::TooManyLines => "more than two lines",
            QcCheck::Empty => "empty cue",
            QcCheck::PastEnd => "ends after the video",
            QcCheck::UncoveredSpeech => "heard speech over 1 s with no cue",
            QcCheck::Unsure => "unsure after re-decode",
            QcCheck::Novel => "word no engine heard",
            QcCheck::RemovedLocked => "agreed word dropped",
            QcCheck::WeakTiming => "timed without the aligner",
            QcCheck::Offset => "aligner offset of 30 ms or more",
            QcCheck::FailedCall => "language-model call failed",
        }
    }

    /// Whether the finding breaks a layout rule the owner's success criteria forbid outright;
    /// the others are for review.
    pub fn is_layout_violation(self) -> bool {
        matches!(
            self,
            QcCheck::Overlap
                | QcCheck::TooShort
                | QcCheck::LineTooLong
                | QcCheck::TooManyLines
                | QcCheck::Empty
                | QcCheck::PastEnd
        )
    }
}

/// One flagged line.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct QcFinding {
    pub check: QcCheck,
    /// Where to look, in video seconds.
    pub time_s: f64,
    /// The cue or utterance text.
    pub text: String,
    /// Numbers behind the finding, such as `23.4 cps` or `U0412`.
    pub detail: String,
    /// The utterance the finding is about, when it is about one line of speech.
    #[serde(default)]
    pub utterance: Option<String>,
}

/// The counts the report opens with.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct QcSummary {
    pub cues: usize,
    pub dialogue_cues: usize,
    pub sound_cues: usize,
    pub music_cues: usize,
    /// Share of cues at or under 20 characters per second (target 95 %).
    pub cps_ok_share: f64,
    /// Words per timing source name.
    pub words_by_source: BTreeMap<String, usize>,
    pub unsure: usize,
    pub novel: usize,
    /// The aligner's signed median offset from the backbone, in milliseconds.
    pub offset_ms: Option<f64>,
    /// Seconds of speech an engine heard words in, outside songs, with no cue on it.
    pub uncovered_speech_s: f64,
    /// Seconds of voice activity outside songs with no cue on it, words heard or not (grunts,
    /// crowds and shouts included).
    #[serde(default)]
    pub voice_without_cue_s: f64,
    pub video_s: f64,
    /// Lines the owner corrected or kept; their unsure, novel and dropped-word findings are
    /// settled.
    #[serde(default)]
    pub reviewed: usize,
    /// Lines Fix It changed that the owner has not checked yet; their words are checked again.
    #[serde(default)]
    pub fixed: usize,
}

/// The quality check's whole result.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct QcReport {
    pub summary: QcSummary,
    /// Every finding, in time order.
    pub findings: Vec<QcFinding>,
}

impl QcReport {
    /// Findings per check.
    pub fn counts(&self) -> BTreeMap<QcCheck, usize> {
        let mut counts = BTreeMap::new();
        for finding in &self.findings {
            *counts.entry(finding.check).or_insert(0) += 1;
        }
        counts
    }

    /// Whether any finding breaks a layout rule outright.
    pub fn has_layout_violations(&self) -> bool {
        self.findings.iter().any(|f| f.check.is_layout_violation())
    }

    /// Why the job does not pass the quality check, one reason per failed rule; empty when it
    /// passes. A job passes with no layout violation, no heard speech left without a cue, no
    /// failed language-model call, no aligner offset of 30 ms or more, and at least
    /// [`CPS_TARGET`] of its cues at or under 20 characters per second. The other findings are
    /// for review and never fail a job.
    pub fn failures(&self) -> Vec<String> {
        let counts = self.counts();
        let mut reasons = Vec::new();
        let layout: usize = counts
            .iter()
            .filter(|(check, _)| check.is_layout_violation())
            .map(|(_, n)| n)
            .sum();
        if layout > 0 {
            reasons.push(format!("{layout} layout rule(s) broken"));
        }
        for check in [
            QcCheck::UncoveredSpeech,
            QcCheck::FailedCall,
            QcCheck::Offset,
        ] {
            if let Some(n) = counts.get(&check) {
                reasons.push(format!("{n} × {}", check.describe()));
            }
        }
        if self.summary.cues > 0 && self.summary.cps_ok_share < CPS_TARGET {
            reasons.push(format!(
                "{:.1} % of cues within 20 characters per second (target {:.0} %)",
                self.summary.cps_ok_share * 100.0,
                CPS_TARGET * 100.0
            ));
        }
        reasons
    }

    /// Whether the job passes the quality check; see [`QcReport::failures`].
    pub fn passes(&self) -> bool {
        self.failures().is_empty()
    }
}

#[cfg(test)]
#[path = "tests/report.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/archive.rs"]
mod archive_tests;
