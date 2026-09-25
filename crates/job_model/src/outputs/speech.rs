//! Where the speech is and how the audio is cut for the speech engines, as kept in `vad.json`.

use serde::{Deserialize, Serialize};

/// The voice-activity result and the chunk plan every engine transcribes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SpeechPlan {
    /// The detector's frame length in seconds.
    pub frame_s: f64,
    /// The score at or above which a frame counts as speech.
    pub threshold: f32,
    /// Speech regions, padded and merged, ascending and never overlapping.
    pub regions: Vec<TimeSpan>,
    /// The chunks, ascending, never overlapping, each cut in a silence where one exists.
    pub chunks: Vec<TimeSpan>,
}

/// A stretch of time in seconds from the start of the video.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeSpan {
    pub start_s: f64,
    pub end_s: f64,
}

impl TimeSpan {
    pub fn new(start_s: f64, end_s: f64) -> TimeSpan {
        TimeSpan { start_s, end_s }
    }

    pub fn duration_s(&self) -> f64 {
        self.end_s - self.start_s
    }

    /// Seconds this span shares with `other`.
    pub fn overlap_s(&self, other: &TimeSpan) -> f64 {
        (self.end_s.min(other.end_s) - self.start_s.max(other.start_s)).max(0.0)
    }
}

impl SpeechPlan {
    /// Total seconds of speech.
    pub fn speech_s(&self) -> f64 {
        self.regions.iter().map(TimeSpan::duration_s).sum()
    }

    /// Seconds of speech inside `span`.
    pub fn speech_within(&self, span: &TimeSpan) -> f64 {
        self.regions.iter().map(|r| r.overlap_s(span)).sum()
    }
}
