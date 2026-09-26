//! Cue building: the aligned words and the chosen sound cues laid out as subtitle cues by the
//! Netflix English rules, on the frames of the source video.
//!
//! **Role:** cut utterances into cues (`segment.rs`), break lines (`line_break.rs`), time each
//! cue from its speech (`timing.rs`), snap to shot cuts (`shots.rs`), place the sound cues
//! (`sound.rs`), and hand back the finished track.
//!
//! **Position:** called by the cue step in the job runner, from `aligned.json`,
//! `sound_cues.json`, `shots.json` and the probe's frame rate; the result is `cues.json`.
//!
//! **Signals and state:** none; pure.
//!
//! **Invariants:** every time is a whole frame; cues come out in time order; when rules clash,
//! speech covered wins, then the 2-frame gap, then the minimum duration, then the shot rules.

pub mod line_break;
pub mod segment;
pub mod shots;
pub mod sound;
pub mod timing;

use job_model::outputs::{Aligned, ShotChanges, SoundCue};
use subtitle_formats::cue::{Cue, CueKind, CueLine, CueTrack, FrameRate};

/// The frame counts the rules are written in, for one frame rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameRules {
    pub rate: FrameRate,
    /// Frames the cue starts before the speech.
    pub lead_in: u64,
    /// Frames the cue stays after the speech (0.5 s).
    pub lead_out: u64,
    /// The shortest cue (5/6 s).
    pub min: u64,
    /// The longest cue (7 s).
    pub max: u64,
    /// The smallest gap between cues.
    pub gap: u64,
    /// Gaps up to this many frames close to `gap` (chaining).
    pub chain_max: u64,
    /// How near a shot cut must be to pull a cue onto it (0.5 s).
    pub shot_window: u64,
    /// Frames in the video; no cue ends after it.
    pub total: u64,
}

impl FrameRules {
    pub fn new(rate: FrameRate, duration_s: f64) -> FrameRules {
        let frames = |s: f64| rate.frame_round(s).max(1);
        let lead_out = frames(0.5);
        FrameRules {
            rate,
            lead_in: 1,
            lead_out,
            min: frames(5.0 / 6.0),
            max: rate.frame_floor(7.0),
            gap: 2,
            chain_max: lead_out.saturating_sub(1),
            shot_window: lead_out,
            total: rate.frame_floor(duration_s),
        }
    }

    /// Frames needed to show `chars` at 20 characters per second.
    pub fn reading_frames(&self, chars: usize) -> u64 {
        self.rate.frame_ceil(chars as f64 / segment::MAX_CPS)
    }
}

/// A cue being built: its lines, the speech it covers, and its frames so far.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    pub lines: Vec<CueLine>,
    pub kind: CueKind,
    pub speech_start_s: f64,
    pub speech_end_s: f64,
    pub start: u64,
    pub end: u64,
}

impl Draft {
    pub fn new(
        lines: Vec<CueLine>,
        kind: CueKind,
        speech_start_s: f64,
        speech_end_s: f64,
    ) -> Draft {
        Draft {
            lines,
            kind,
            speech_start_s,
            speech_end_s,
            start: 0,
            end: 0,
        }
    }

    pub fn chars(&self) -> usize {
        self.lines.iter().map(CueLine::chars).sum()
    }

    /// The first frame of speech and the frame after the last.
    pub fn speech_frames(&self, rate: FrameRate) -> (u64, u64) {
        (
            rate.frame_floor(self.speech_start_s),
            rate.frame_ceil(self.speech_end_s),
        )
    }
}

/// What the cue step builds: the track, and the sound cues that found no place.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    pub track: CueTrack,
    pub dropped_sounds: Vec<String>,
}

/// Build the cue track.
pub fn build(
    aligned: &Aligned,
    sounds: &[SoundCue],
    shot_changes: &ShotChanges,
    cut_score: f64,
    rate: FrameRate,
    duration_s: f64,
) -> Built {
    let rules = FrameRules::new(rate, duration_s);
    let units = segment::units(aligned);
    let mut drafts = segment::drafts(&units, &rules);
    drafts.sort_by(|a, b| a.speech_start_s.total_cmp(&b.speech_start_s));
    timing::initial(&mut drafts, &rules);
    let cuts = shots::cut_frames(shot_changes, cut_score, rate);
    shots::snap(&mut drafts, &cuts, &rules);
    for _ in 0..2 {
        timing::extend(&mut drafts, &rules);
        timing::separate(&mut drafts, &rules);
    }
    let dropped_sounds = sound::place(&mut drafts, sounds, &rules);
    timing::chain(&mut drafts, &rules);
    timing::clamp(&mut drafts, &rules);
    let cues = drafts
        .into_iter()
        .map(|d| Cue {
            start: d.start,
            end: d.end,
            lines: d.lines,
            kind: d.kind,
        })
        .collect();
    Built {
        track: CueTrack {
            frame_rate: rate,
            cues,
        },
        dropped_sounds,
    }
}

#[cfg(test)]
#[path = "tests/build.rs"]
mod tests;
