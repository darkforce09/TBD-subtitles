//! What a speech engine heard: words with times, per chunk, as kept in `outputs/asr_<engine>`.

use serde::{Deserialize, Serialize};

use super::speech::TimeSpan;

/// One word with its time in seconds from the start of the video.
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
pub struct TimedWord {
    /// The word as the engine wrote it, with any punctuation attached.
    pub text: String,
    pub start_s: f64,
    pub end_s: f64,
    /// The engine's probability for the word, when it gives one.
    pub confidence: Option<f32>,
}

/// One engine's words over the chunk plan.
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
pub struct EngineTranscript {
    /// The engine and model, such as `parakeet-tdt-0.6b-v2`.
    pub engine: String,
    /// The audio it heard, such as `mix` or `vocals.roformer`.
    pub input: String,
    pub chunks: Vec<ChunkWords>,
}

/// The words heard in one chunk.
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
pub struct ChunkWords {
    pub span: TimeSpan,
    pub words: Vec<TimedWord>,
}

impl EngineTranscript {
    /// Every word, in time order.
    pub fn words(&self) -> impl Iterator<Item = &TimedWord> {
        self.chunks.iter().flat_map(|c| c.words.iter())
    }

    /// The text of every word, joined by spaces.
    pub fn text(&self) -> String {
        self.words()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}
