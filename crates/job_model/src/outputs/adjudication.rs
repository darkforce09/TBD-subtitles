//! The language model's answers and the checks on them, as kept in `adjudication/first.json`
//! and `adjudicated.json`; and the unsure utterances heard again, as kept in
//! `adjudication/redecode_<engine>.json`.
//!
//! **Role:** carry each adjudication pass and each re-decode from the adjudication steps to the
//! alignment, the quality check and Fix It.
//!
//! **Position:** written by the adjudication stage; read by the later stages and the window.
//!
//! **Signals and state:** none; plain data, written as JSON and archived with rkyv.
//!
//! **Invariants:** a pass holds one line per utterance of the sheet it was asked about, in sheet
//! order; a re-decode holds one chunk per id, in the same order.

use serde::{Deserialize, Serialize};

use super::words::EngineTranscript;

/// One adjudicated utterance, as the model returns it: final text (`||` marks a speaker change)
/// and flags (`NARR`, `LYRIC`, `DROP`, `UNSURE`).
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
pub struct Line {
    pub id: String,
    pub t: String,
    #[serde(default)]
    pub f: Vec<String>,
}

impl Line {
    pub fn has_flag(&self, flag: &str) -> bool {
        self.f.iter().any(|f| f == flag)
    }
}

/// What the checks found.
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
pub struct Findings {
    pub missing_ids: Vec<String>,
    pub duplicate_ids: Vec<String>,
    pub unknown_ids: Vec<String>,
    /// `(id, word)` for each output word no engine heard nearby and the glossary lacks.
    pub novel: Vec<(String, String)>,
    /// `(id, word)` for each agreed, non-filler word the answer dropped.
    pub removed_locked: Vec<(String, String)>,
    /// Ids whose text reads faster than the check's limit.
    pub too_fast: Vec<String>,
}

/// One pass of the language model over (part of) the sheet, with the checks on its answer.
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
pub struct AdjudicationPass {
    /// One line per utterance of the sheet, in sheet order.
    pub lines: Vec<Line>,
    pub findings: Findings,
    /// The ids asked again with re-decoded alternatives.
    #[serde(default)]
    pub redecoded: Vec<String>,
    pub calls: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    /// Why a call failed, per failed call.
    #[serde(default)]
    pub failed_calls: Vec<String>,
}

/// The unsure utterances heard again: one chunk per id, in the same order.
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
pub struct Redecode {
    pub ids: Vec<String>,
    pub transcript: EngineTranscript,
}
