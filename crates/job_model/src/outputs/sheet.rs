//! The diff sheet's utterances, as kept in `outputs/diff_sheet`: the backbone engine's words cut at
//! pauses, every other engine's hypothesis, and the line the language model reads.

use serde::{Deserialize, Serialize};

use super::words::TimedWord;

/// One utterance of the sheet.
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
pub struct Utterance {
    pub id: String,
    pub start_s: f64,
    pub end_s: f64,
    /// The backbone's words.
    pub words: Vec<TimedWord>,
    /// Whether every engine heard each backbone word the same (after normalising).
    pub locked: Vec<bool>,
    /// The sheet line.
    pub line: String,
    /// Each engine's words for this utterance, backbone first: `(engine tag, words)`.
    pub hypotheses: Vec<(String, Vec<String>)>,
}
