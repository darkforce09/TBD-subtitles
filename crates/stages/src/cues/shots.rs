//! Shot cuts: which scdet changes count as cuts, and how cue edges move onto them.
//!
//! **Role:** keep the changes scoring at least the cut score, merge changes closer than half a
//! second (flashes, impact frames) into the strongest; then start a cue on a cut its speech
//! follows within `shot_window` frames, end a cue `gap` frames before a cut near its end, and
//! keep a cue from crossing a cut by fewer than `shot_window` frames.
//!
//! **Position:** called by `mod.rs` after the first timing, before the passes that outrank it.
//!
//! **Signals and state:** none; the drafts are changed in place.
//!
//! **Invariants:** a moved end never cuts speech; the cut list is sorted and holds no two cuts
//! within half a second.

use job_model::outputs::ShotChanges;
use subtitle_formats::cue::FrameRate;

use super::{Draft, FrameRules};

/// Changes closer than this are one cut.
pub const MERGE_S: f64 = 0.5;

/// The frames of the cuts scoring at least `min_score`, close changes merged into the strongest.
pub fn cut_frames(shots: &ShotChanges, min_score: f64, rate: FrameRate) -> Vec<u64> {
    let mut kept: Vec<(f64, f64)> = Vec::new();
    let mut cuts: Vec<(f64, f64)> = shots
        .cuts
        .iter()
        .filter(|c| c.score >= min_score)
        .map(|c| (c.time_s, c.score))
        .collect();
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (time, score) in cuts {
        match kept.last_mut() {
            Some(last) if time - last.0 < MERGE_S => {
                if score > last.1 {
                    *last = (time, score);
                }
            }
            _ => kept.push((time, score)),
        }
    }
    let mut frames: Vec<u64> = kept.into_iter().map(|(t, _)| rate.frame_round(t)).collect();
    frames.dedup();
    frames
}

/// Move cue edges onto the cuts.
pub fn snap(drafts: &mut [Draft], cuts: &[u64], rules: &FrameRules) {
    let w = rules.shot_window;
    for d in drafts {
        let (onset, done) = d.speech_frames(rules.rate);
        // Speech starts within `w` frames after a cut: start on the cut.
        if let Some(&c) = cuts.iter().rev().find(|&&c| c <= onset && onset - c <= w) {
            d.start = c;
        } else if let Some(&c) = cuts
            .iter()
            .find(|&&c| c > d.start && c < d.start + w && c > onset)
        {
            // Speech starts just before a cut: start early enough not to flash across it.
            d.start = c.saturating_sub(w).min(d.start);
        }
        // A cut near the end: end `gap` before it, or clear it by `w` if the speech runs past.
        if let Some(&c) = cuts
            .iter()
            .find(|&&c| c.abs_diff(d.end) <= w && c > d.start)
        {
            if c >= done + rules.gap {
                d.end = c - rules.gap;
            } else if d.end > c && d.end < c + w {
                d.end = c + w;
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/shots.rs"]
mod tests;
