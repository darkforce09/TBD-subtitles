//! Fix It's record, `fix.json`: what the model worked out about the video, every line it was
//! asked about with each change it proposed, what the guard and the judge made of it, what the
//! calls cost, and the problems before the first run.
//!
//! **Role:** keep what every Fix It run of a video answered, so the owner can see why a line
//! changed, the window can mark the changed lines, and a later run does not ask again what an
//! earlier one answered.
//!
//! **Position:** written by `pipeline::fix_it`, which reads it back to carry earlier answers
//! into the next run; read by the window's line review and its report.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** a line is changed only when its verdict writes a correction; `applied` is set
//! only for a correction that reached `review.json`; a record whose `adjudication` differs from
//! the job's re-adjudication is not current, and nothing in it counts.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::StepName;
use crate::job::JobRecord;
use crate::report::{QcCheck, QcReport};

/// The kinds of problem Fix It asks about, in the order it asks.
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
pub enum FixFamily {
    /// Unsure lines, words no engine heard, heard words replaced, and lines that do not fit.
    Words,
    /// Speech with no subtitle, loosely timed lines, and subtitles that break a layout rule.
    Timing,
    /// Subtitles over 20 characters per second.
    ReadingSpeed,
}

impl FixFamily {
    pub const ALL: [FixFamily; 3] = [FixFamily::Words, FixFamily::Timing, FixFamily::ReadingSpeed];

    /// The family's name in a sentence.
    pub fn describe(self) -> &'static str {
        match self {
            FixFamily::Words => "words",
            FixFamily::Timing => "timing and layout",
            FixFamily::ReadingSpeed => "reading speed",
        }
    }
}

/// A line the first pass found out of place in the conversation.
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
pub struct Suspect {
    pub id: String,
    pub why: String,
}

/// What the first pass worked out about the video from its names and its lines.
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
pub struct FixBrief {
    /// The series, such as "One Piece (anime), English dub".
    pub show: String,
    /// The arc and episode, as far as the names tell.
    pub episode: String,
    /// The characters who speak, as they are named.
    pub cast: Vec<String>,
    /// Who speaks to whom, scene by scene.
    pub summary: String,
    /// Ways of speaking that are meant: a stutter, a catchphrase, a repeated word.
    pub speech_habits: Vec<String>,
    pub suspects: Vec<Suspect>,
}

/// One family's change to a line, as it passed the guard.
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
pub struct FixStep {
    pub family: FixFamily,
    pub text: String,
    pub flags: Vec<String>,
    pub why: String,
}

/// What became of a line Fix It asked about.
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
#[serde(rename_all = "snake_case")]
pub enum FixVerdict {
    /// Nothing to write: no answer, or no change worth keeping.
    Unchanged,
    /// Its words and flags stay; it is re-timed alone, or the model confirmed it.
    Kept { why: String },
    /// The judge accepted the change.
    Accepted { why: String },
    /// The judge turned the change down.
    TurnedDown { why: String },
    /// The judge gave no verdict: its answer left the line out, or its call failed.
    NotJudged { why: String },
    /// Every call that asked about the line failed, so the model never answered it.
    NotAnswered { why: String },
}

impl FixVerdict {
    /// Whether the line gets a Fix It correction.
    pub fn writes_correction(&self) -> bool {
        matches!(self, FixVerdict::Kept { .. } | FixVerdict::Accepted { .. })
    }

    /// Whether the model answered the line and the answer came to a verdict, so a later run does
    /// not ask about it again.
    pub fn answered(&self) -> bool {
        matches!(
            self,
            FixVerdict::Unchanged
                | FixVerdict::Kept { .. }
                | FixVerdict::Accepted { .. }
                | FixVerdict::TurnedDown { .. }
        )
    }
}

/// One line Fix It asked about.
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
pub struct LineFix {
    pub id: String,
    /// The problems it was asked about, in words.
    pub problems: Vec<String>,
    /// The quality checks it was asked about; empty in a record that predates them, which then
    /// covers every check of the line.
    #[serde(default)]
    pub checks: Vec<QcCheck>,
    pub before_text: String,
    pub before_flags: Vec<String>,
    pub after_text: String,
    pub after_flags: Vec<String>,
    /// Each family's change that passed the guard, in the order asked.
    #[serde(default)]
    pub steps: Vec<FixStep>,
    /// Each proposal the guard refused, with the rule it broke.
    #[serde(default)]
    pub refused: Vec<String>,
    /// Heard words the change leaves out.
    #[serde(default)]
    pub removed: Vec<String>,
    pub verdict: FixVerdict,
    /// Whether its correction reached `review.json`; the owner's own correction wins.
    #[serde(default)]
    pub applied: bool,
}

impl LineFix {
    /// Whether the text or the flags differ from before.
    pub fn changed(&self) -> bool {
        self.after_text != self.before_text || self.after_flags != self.before_flags
    }

    /// Why the line got its correction, as the owner reads it.
    pub fn why(&self) -> String {
        match &self.verdict {
            FixVerdict::Kept { why } => why.clone(),
            _ => self
                .steps
                .iter()
                .map(|s| s.why.trim())
                .filter(|w| !w.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
        }
    }
}

/// The quality check's problems before the first Fix It run.
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
pub struct FixBefore {
    /// Findings per check.
    pub counts: BTreeMap<QcCheck, usize>,
    /// Where the first speech with no subtitle starts, in video seconds.
    pub first_uncovered_s: Option<f64>,
    /// Share of cues at or under 20 characters per second.
    pub cps_ok_share: f64,
    pub cues: usize,
}

impl FixBefore {
    /// The problems `qc` names.
    pub fn of(qc: &QcReport) -> FixBefore {
        FixBefore {
            counts: qc.counts(),
            first_uncovered_s: qc
                .findings
                .iter()
                .filter(|f| f.check == QcCheck::UncoveredSpeech)
                .map(|f| f.time_s)
                .min_by(f64::total_cmp),
            cps_ok_share: qc.summary.cps_ok_share,
            cues: qc.summary.cues,
        }
    }
}

/// Every Fix It run of a video: the latest run's lines with the lines earlier runs answered, and
/// what all the calls cost.
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
pub struct FixRecord {
    /// The `claude` model, such as `opus`.
    pub model: String,
    /// The video's file name without its extension.
    pub video: String,
    pub brief: FixBrief,
    /// Each line asked about, by id.
    pub lines: Vec<LineFix>,
    pub calls: usize,
    /// Calls answered from an earlier, stopped run.
    #[serde(default)]
    pub cached_calls: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    /// Why a call failed, per failed call.
    #[serde(default)]
    pub failed_calls: Vec<String>,
    /// The fingerprint of the job's re-adjudication the runs read; empty when unknown.
    #[serde(default)]
    pub adjudication: String,
    /// The problems before the first Fix It run.
    #[serde(default)]
    pub before: Option<FixBefore>,
}

impl FixRecord {
    /// Whether the record belongs to `job`'s re-adjudication as it stands: its `adjudication` is
    /// empty, or matches the fingerprint of the job's re-adjudication step.
    pub fn is_current(&self, job: &JobRecord) -> bool {
        self.adjudication.is_empty()
            || job
                .steps
                .get(&StepName::Readjudicate)
                .is_some_and(|step| step.fingerprint == self.adjudication)
    }
}

#[cfg(test)]
#[path = "tests/fix_it.rs"]
mod tests;
