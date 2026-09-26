//! Sound and music cues placed among the dialogue cues.
//!
//! **Role:** give a sound cue a cue of its own where the dialogue leaves a free stretch of at
//! least 0.8 s around it; otherwise add it as its own line to the dialogue cue it overlaps when
//! that cue has one line and the text fits; otherwise drop it and say so. A song's music cue is
//! placed the same way and is never longer than 7 s.
//!
//! **Position:** called by `mod.rs` once the dialogue cues are timed and separated.
//!
//! **Signals and state:** none; the drafts are changed in place and kept in time order.
//!
//! **Invariants:** a placed cue never overlaps another or comes within `gap` frames of it;
//! sound lines are never italic.

use job_model::outputs::{CandidateKind, SoundCue};
use subtitle_formats::cue::{CueKind, CueLine};

use super::line_break::MAX_LINE;
use super::{Draft, FrameRules};

/// The smallest free stretch a sound cue takes on its own.
pub const MIN_FREE_S: f64 = 0.8;
/// How far after its sound a cue may still be shown.
pub const LATE_S: f64 = 0.5;

/// Place every sound cue; returns a description of each one dropped.
pub fn place(drafts: &mut Vec<Draft>, sounds: &[SoundCue], rules: &FrameRules) -> Vec<String> {
    let mut dropped = Vec::new();
    let mut sorted: Vec<&SoundCue> = sounds.iter().collect();
    sorted.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));
    for sound in sorted {
        if !own_cue(drafts, sound, rules) && !shared_line(drafts, sound, rules) {
            dropped.push(format!("{:.1}s {}", sound.start_s, sound.text));
        }
    }
    dropped
}

/// Put the sound in a free stretch that overlaps it; `false` when there is none.
fn own_cue(drafts: &mut Vec<Draft>, sound: &SoundCue, rules: &FrameRules) -> bool {
    let rate = rules.rate;
    let from = rate.frame_floor(sound.start_s);
    let until = rate.frame_ceil(sound.end_s.max(sound.start_s) + LATE_S);
    let min_free = rate.frame_ceil(MIN_FREE_S).max(rules.min);
    let kind = if sound.kind == CandidateKind::Song {
        CueKind::Music
    } else {
        CueKind::Sound
    };
    let wanted = rules
        .min
        .max(rules.reading_frames(sound.text.chars().count()))
        .max(until.saturating_sub(from + rate.frame_ceil(LATE_S)))
        .min(rules.max);
    // Free stretches: before the first cue, between cues, after the last.
    let mut best: Option<(u64, u64)> = None;
    for i in 0..=drafts.len() {
        let open = if i == 0 {
            0
        } else {
            drafts[i - 1].end + rules.gap
        };
        let close = drafts
            .get(i)
            .map_or(rules.total, |d| d.start.saturating_sub(rules.gap));
        if close <= open || close - open < min_free {
            continue;
        }
        let overlap = close.min(until).saturating_sub(open.max(from));
        if overlap > 0
            && best.is_none_or(|(o, c)| overlap > c.min(until).saturating_sub(o.max(from)))
        {
            best = Some((open, close));
        }
    }
    let Some((open, close)) = best else {
        return false;
    };
    let start = from
        .max(open)
        .min(close.saturating_sub(rules.min).max(open));
    let end = (start + wanted).min(close);
    let draft = Draft {
        lines: vec![CueLine::plain(sound.text.clone())],
        kind,
        speech_start_s: sound.start_s,
        speech_end_s: sound.start_s,
        start,
        end,
        starts_speaker: false,
    };
    let at = drafts.partition_point(|d| d.start < start);
    drafts.insert(at, draft);
    true
}

/// Add the sound as a line of the one-line dialogue cue it overlaps; `false` when none fits.
fn shared_line(drafts: &mut [Draft], sound: &SoundCue, rules: &FrameRules) -> bool {
    if sound.text.chars().count() > MAX_LINE || sound.kind == CandidateKind::Song {
        return false;
    }
    let from = rules.rate.frame_floor(sound.start_s);
    let until = rules.rate.frame_ceil(sound.end_s.max(sound.start_s + 0.1));
    let Some(d) = drafts.iter_mut().find(|d| {
        d.kind == CueKind::Dialogue && d.lines.len() == 1 && d.start < until && d.end > from
    }) else {
        return false;
    };
    let line = CueLine::plain(sound.text.clone());
    if sound.start_s < d.speech_start_s {
        d.lines.insert(0, line);
    } else {
        d.lines.push(line);
    }
    true
}

#[cfg(test)]
#[path = "tests/sound.rs"]
mod tests;
