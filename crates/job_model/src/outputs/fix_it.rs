//! Fix It's record, `fix.json`: what the model worked out about the video, every line it was
//! asked about with each change it proposed, what the guard and the judge made of it, and what
//! the calls cost.
//!
//! **Role:** keep the whole of one Fix It run, so the owner can see why a line changed and the
//! window can mark the changed lines.
//!
//! **Position:** written by `pipeline::fix_it`; read by the window's line review and report.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** a line is changed only when its verdict writes a correction; `applied` is set
//! only for a correction that reached `review.json`.

use serde::{Deserialize, Serialize};

/// The kinds of problem Fix It asks about, in the order it asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Suspect {
    pub id: String,
    pub why: String,
}

/// What the first pass worked out about the video from its names and its lines.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixStep {
    pub family: FixFamily,
    pub text: String,
    pub flags: Vec<String>,
    pub why: String,
}

/// What became of a line Fix It asked about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
}

impl FixVerdict {
    /// Whether the line gets a Fix It correction.
    pub fn writes_correction(&self) -> bool {
        matches!(self, FixVerdict::Kept { .. } | FixVerdict::Accepted { .. })
    }
}

/// One line Fix It asked about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineFix {
    pub id: String,
    /// The problems it was asked about, in words.
    pub problems: Vec<String>,
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

/// One whole Fix It run of a video.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FixRecord {
    /// The `claude` model, such as `opus`.
    pub model: String,
    /// The video's file name without its extension.
    pub video: String,
    pub brief: FixBrief,
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
}

#[cfg(test)]
#[path = "tests/fix_it.rs"]
mod tests;
