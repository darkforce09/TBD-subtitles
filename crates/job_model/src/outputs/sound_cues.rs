//! The sound-cue candidates and the ones the language model chose and worded, as kept in
//! `sound_cues.json`.
//!
//! **Role:** carry the sound candidates and the chosen cues from the sound-cue step to the cue
//! layout and the quality check.
//!
//! **Position:** written by the sound-cue step of the adjudication stage; read by the cues stage.
//!
//! **Signals and state:** none; plain data, written as JSON and archived with rkyv.
//!
//! **Invariants:** every cue names the candidate it was chosen from and keeps its time.

use serde::{Deserialize, Serialize};

/// Where a candidate came from.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[rkyv(compare(PartialEq), derive(Debug, PartialEq, Eq))]
pub enum CandidateKind {
    /// A sound effect on the background stem.
    Effect,
    /// A non-speech voice on the vocal stem (a gasp, a scream).
    Voice,
    /// A sound Whisper wrote as a tag, such as `*Grunting*`.
    Tag,
    /// A song: sung lyrics the language model flagged, with the music around them.
    Song,
}

/// One sound the language model may turn into a cue.
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
pub struct SoundCandidate {
    /// `S001`, `S002`, …
    pub id: String,
    pub kind: CandidateKind,
    /// The AudioSet class, the Whisper tag, or `Song`.
    pub label: String,
    pub start_s: f64,
    pub end_s: f64,
    /// The detector's peak probability; 1 for tags and songs.
    pub peak: f32,
}

/// A chosen, worded sound cue, at its candidate's time.
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
pub struct SoundCue {
    pub candidate: String,
    pub kind: CandidateKind,
    pub start_s: f64,
    pub end_s: f64,
    /// Bracketed and lowercase except proper nouns, such as `[explosion]`.
    pub text: String,
}

/// The candidates, the chosen cues and the song spans.
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
pub struct SoundCues {
    pub candidates: Vec<SoundCandidate>,
    pub cues: Vec<SoundCue>,
    /// Answers the checks refused, with the reason.
    #[serde(default)]
    pub refused: Vec<String>,
    #[serde(default)]
    pub failed_calls: Vec<String>,
    pub cost_usd: f64,
}
