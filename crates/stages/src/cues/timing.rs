//! Cue timing from speech: lead-in and lead-out, minimum duration and reading speed, the gap
//! between cues, chaining of short gaps, and the end of the video.
//!
//! **Role:** each pass moves cue edges by one rule; `mod.rs` runs them in the order that gives
//! the rule priority (speech covered, then the gap, then the minimum duration, then shots).
//!
//! **Position:** called by `mod.rs` on drafts sorted by speech start.
//!
//! **Signals and state:** none; the drafts are changed in place.
//!
//! **Invariants:** after `separate`, no two cues overlap and every gap is at least `gap` frames,
//! even when a cue must give up part of its lead-out; a cue's start never moves after its speech
//! starts except to keep the gap.

use super::{Draft, FrameRules};

/// In-time one frame before the speech, out-time `lead_out` after it.
pub fn initial(drafts: &mut [Draft], rules: &FrameRules) {
    for d in drafts {
        let (onset, done) = d.speech_frames(rules.rate);
        d.start = onset.saturating_sub(rules.lead_in);
        d.end = done.max(onset + 1) + rules.lead_out;
    }
}

/// Lengthen cues that are too short or too fast: first into the gap after them, then (by at most
/// `lead_out` frames) before them, taking back the previous cue's lead-out when it has more than
/// its speech and its own minimum; shorten cues over `max` down to their speech.
pub fn extend(drafts: &mut [Draft], rules: &FrameRules) {
    for i in 0..drafts.len() {
        let next_start = drafts
            .get(i + 1)
            .map_or(rules.total, |n| n.start.saturating_sub(rules.gap));
        let needed = frames_needed(&drafts[i], rules);
        let (onset, done) = drafts[i].speech_frames(rules.rate);
        let d = &mut drafts[i];
        if d.end.saturating_sub(d.start) < needed {
            d.end = d.end.max((d.start + needed).min(next_start));
        }
        let short = needed.saturating_sub(drafts[i].end.saturating_sub(drafts[i].start));
        if short > 0 {
            let earliest = onset.saturating_sub(rules.lead_out);
            let wanted = drafts[i].start.saturating_sub(short).max(earliest);
            if i > 0 {
                let prev = &mut drafts[i - 1];
                let (_, prev_done) = prev.speech_frames(rules.rate);
                let prev_floor = (prev.start + frames_needed(prev, rules)).max(prev_done);
                let released = wanted.saturating_sub(rules.gap).max(prev_floor);
                prev.end = prev.end.min(released.max(prev.start + 1));
            }
            let prev_end = if i == 0 {
                0
            } else {
                drafts[i - 1].end + rules.gap
            };
            let d = &mut drafts[i];
            d.start = wanted.max(prev_end).min(d.start);
        }
        let d = &mut drafts[i];
        if d.end.saturating_sub(d.start) > rules.max {
            d.end = (d.start + rules.max).max(done);
        }
    }
}

/// Frames a cue needs: the minimum, or longer for 20 characters per second, at most `max`.
fn frames_needed(d: &Draft, rules: &FrameRules) -> u64 {
    rules
        .min
        .max(rules.reading_frames(d.chars()))
        .min(rules.max)
}

/// Keep `gap` frames between cues: a cue ends `gap` before the next starts; when that would cut
/// its speech, it keeps its speech and the next cue starts later instead.
pub fn separate(drafts: &mut [Draft], rules: &FrameRules) {
    for i in 1..drafts.len() {
        let (before, after) = drafts.split_at_mut(i);
        let prev = &mut before[i - 1];
        let next = &mut after[0];
        if prev.end + rules.gap <= next.start {
            continue;
        }
        let (_, prev_done) = prev.speech_frames(rules.rate);
        let limit = next.start.saturating_sub(rules.gap);
        prev.end = limit.max(prev_done.min(prev.end)).max(prev.start + 1);
        if prev.end + rules.gap > next.start {
            next.start = prev.end + rules.gap;
            next.end = next.end.max(next.start + 1);
        }
    }
}

/// Close gaps of 3 to `chain_max` frames to `gap` by holding the earlier cue longer.
pub fn chain(drafts: &mut [Draft], rules: &FrameRules) {
    for i in 1..drafts.len() {
        let next_start = drafts[i].start;
        let prev = &mut drafts[i - 1];
        let gap = next_start.saturating_sub(prev.end);
        let held = next_start - rules.gap.min(next_start);
        if gap > rules.gap && gap <= rules.chain_max && held.saturating_sub(prev.start) <= rules.max
        {
            prev.end = held;
        }
    }
}

/// No cue ends after the video, and every cue lasts at least one frame.
pub fn clamp(drafts: &mut [Draft], rules: &FrameRules) {
    for d in drafts {
        d.end = d.end.min(rules.total.max(1));
        d.start = d.start.min(d.end.saturating_sub(1));
    }
}

#[cfg(test)]
#[path = "tests/timing.rs"]
mod tests;
