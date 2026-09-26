//! The sound-cue candidates and the ones the language model chose and worded, as kept in
//! `sound_cues.json`.

use serde::{Deserialize, Serialize};

/// Where a candidate came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoundCue {
    pub candidate: String,
    pub kind: CandidateKind,
    pub start_s: f64,
    pub end_s: f64,
    /// Bracketed and lowercase except proper nouns, such as `[explosion]`.
    pub text: String,
}

/// The candidates, the chosen cues and the song spans.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
