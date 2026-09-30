//! The final words with their times and where each time came from, as kept in `aligned.json`.
//!
//! **Role:** carry the timed final words from the alignment to the cues and the quality check.
//!
//! **Position:** written by the alignment stage and the review step; read by the cue layout and
//! the quality check.
//!
//! **Signals and state:** none; plain data, written as JSON and archived with rkyv.
//!
//! **Invariants:** utterances are in time order; every word names the source that timed it.

use serde::{Deserialize, Serialize};

/// Which source timed a word, best first.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[rkyv(compare(PartialEq), derive(Debug, PartialEq, Eq, PartialOrd, Ord))]
pub enum TimingSource {
    /// The CTC aligner, over a whole block.
    Ctc,
    /// The CTC aligner, over the utterance alone, after its block failed the checks.
    CtcUtterance,
    /// The backbone engine's own time for the same word.
    Backbone,
    /// Spread between the neighbouring timed words: no source heard the word.
    Interpolated,
}

impl TimingSource {
    pub const ALL: [TimingSource; 4] = [
        TimingSource::Ctc,
        TimingSource::CtcUtterance,
        TimingSource::Backbone,
        TimingSource::Interpolated,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            TimingSource::Ctc => "ctc",
            TimingSource::CtcUtterance => "ctc_utterance",
            TimingSource::Backbone => "backbone",
            TimingSource::Interpolated => "interpolated",
        }
    }
}

/// One displayed word with its time in video seconds.
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
pub struct AlignedWord {
    pub text: String,
    pub start_s: f64,
    pub end_s: f64,
    pub source: TimingSource,
}

/// One kept utterance's final words.
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
pub struct AlignedUtterance {
    pub id: String,
    pub words: Vec<AlignedWord>,
    /// Indices of the words where another speaker starts (the `||` of the answer).
    #[serde(default)]
    pub speaker_starts: Vec<usize>,
    /// The language model marked it as started by a different speaker than the one before.
    #[serde(default)]
    pub new_speaker: bool,
    /// The narrator speaks it; shown in italics.
    #[serde(default)]
    pub narrator: bool,
    /// The language model could not settle it, even after the re-decode.
    #[serde(default)]
    pub unsure: bool,
}

/// Every kept utterance, in time order, and the alignment's own measures.
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
pub struct Aligned {
    pub utterances: Vec<AlignedUtterance>,
    pub blocks: usize,
    /// Blocks that failed the checks and were aligned utterance by utterance.
    pub failed_blocks: usize,
    /// The signed median of aligner start minus backbone start, over words both timed.
    pub offset_s: Option<f64>,
    /// Aligner errors, one per block or utterance that could not be aligned at all.
    #[serde(default)]
    pub errors: Vec<String>,
}
