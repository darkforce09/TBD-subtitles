//! The cue model: a frame rate, and cues of one or two lines with their times in frames.
//!
//! **Role:** hold every finished cue as whole frames of the source video, so no stage or writer
//! can place a time between frames, and convert frames to seconds and milliseconds.
//!
//! **Position:** built by the cue stage (`crates/stages/src/cues/`), checked by the quality
//! check, written by `crate::writers`; stored as `cues.json` in the job's work directory, and
//! archived with rkyv for the job database.
//!
//! **Signals and state:** none; plain values, written as JSON and archived with rkyv.
//!
//! **Invariants:** a frame rate never has a zero numerator or denominator; a cue's times are
//! frame indices, so every time sits on a frame boundary.

use serde::{Deserialize, Serialize};

/// A frame rate as a fraction, such as 24/1 or 24000/1001.
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
pub struct FrameRate {
    num: u32,
    den: u32,
}

impl FrameRate {
    /// Film: exactly 24 frames per second.
    pub const FILM: FrameRate = FrameRate { num: 24, den: 1 };

    /// `num / den` frames per second; `None` when either is zero.
    pub fn new(num: u32, den: u32) -> Option<FrameRate> {
        (num != 0 && den != 0).then_some(FrameRate { num, den })
    }

    pub fn num(self) -> u32 {
        self.num
    }

    pub fn den(self) -> u32 {
        self.den
    }

    /// Frames per second.
    pub fn fps(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    /// The length of one frame in seconds.
    pub fn frame_s(self) -> f64 {
        self.den as f64 / self.num as f64
    }

    /// The time at which `frame` starts, in seconds.
    pub fn seconds(self, frame: u64) -> f64 {
        frame as f64 * self.frame_s()
    }

    /// The time at which `frame` starts, in whole milliseconds, rounded to the nearest.
    pub fn millis(self, frame: u64) -> u64 {
        let scaled = frame as u128 * 1000 * self.den as u128;
        ((scaled * 2 + self.num as u128) / (2 * self.num as u128)) as u64
    }

    /// The last frame boundary at or before `seconds` (0 for negative times).
    pub fn frame_floor(self, seconds: f64) -> u64 {
        (self.frames(seconds) + 1e-6).floor().max(0.0) as u64
    }

    /// The first frame boundary at or after `seconds` (0 for negative times).
    pub fn frame_ceil(self, seconds: f64) -> u64 {
        (self.frames(seconds) - 1e-6).ceil().max(0.0) as u64
    }

    /// The frame boundary nearest to `seconds` (0 for negative times).
    pub fn frame_round(self, seconds: f64) -> u64 {
        self.frames(seconds).round().max(0.0) as u64
    }

    fn frames(self, seconds: f64) -> f64 {
        seconds * self.num as f64 / self.den as f64
    }
}

/// What a cue carries.
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
pub enum CueKind {
    /// Spoken lines, possibly with a sound line added.
    Dialogue,
    /// One bracketed sound description.
    Sound,
    /// One bracketed music description, standing for a song's lyrics.
    Music,
}

/// One displayed line.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct CueLine {
    pub text: String,
    pub italic: bool,
}

impl CueLine {
    pub fn plain(text: impl Into<String>) -> CueLine {
        CueLine {
            text: text.into(),
            italic: false,
        }
    }

    pub fn italic(text: impl Into<String>) -> CueLine {
        CueLine {
            text: text.into(),
            italic: true,
        }
    }

    /// Characters on the line, as a viewer counts them.
    pub fn chars(&self) -> usize {
        self.text.chars().count()
    }
}

/// One subtitle event: shown from frame `start` up to, not including, frame `end`.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct Cue {
    pub start: u64,
    pub end: u64,
    pub lines: Vec<CueLine>,
    pub kind: CueKind,
}

impl Cue {
    /// Frames on screen.
    pub fn frames(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// Characters over all lines, line breaks not counted.
    pub fn chars(&self) -> usize {
        self.lines.iter().map(CueLine::chars).sum()
    }

    /// Characters per second at `rate`; 0 for a cue with no duration.
    pub fn cps(&self, rate: FrameRate) -> f64 {
        let seconds = self.frames() as f64 * rate.frame_s();
        if seconds <= 0.0 {
            0.0
        } else {
            self.chars() as f64 / seconds
        }
    }

    /// The lines joined by a space, for reports.
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Every cue of one video, in time order, with the video's frame rate.
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
pub struct CueTrack {
    pub frame_rate: FrameRate,
    pub cues: Vec<Cue>,
}

#[cfg(test)]
#[path = "tests/cue.rs"]
mod tests;
