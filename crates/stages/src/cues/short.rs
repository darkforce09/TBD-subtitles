//! Cues still too short once timed: a one-word interjection squeezed between two cues that need
//! all their time.
//!
//! **Role:** give each dialogue cue under the minimum a place: share a neighbouring cue (the text
//! broken again into at most two lines, or as `-Line` / `-Line` when the language model marked a
//! speaker change between them), else grow into the time around it, taking back a neighbour's
//! lead-out down to its speech and its own minimum.
//!
//! **Position:** called by `mod.rs` after the timing passes and before the sound cues are placed.
//!
//! **Signals and state:** none; the drafts are changed in place.
//!
//! **Invariants:** no word is lost or reordered; two speakers never share a cue without the
//! dashes; a cue never starts after its speech or ends before it; the 2-frame gap holds. A shared
//! cue keeps to 20 characters per second when it can; when neither sharing within the reading
//! speed nor growing gives the minimum, sharing wins, since a fast cue is for review and a short
//! one breaks a rule.

use subtitle_formats::cue::{CueKind, CueLine};

use super::line_break::{self, MAX_LINE};
use super::segment::MAX_CPS;
use super::{Draft, FrameRules};

/// Place every dialogue cue shorter than the minimum.
pub fn resolve(drafts: &mut Vec<Draft>, rules: &FrameRules) {
    let mut i = 0;
    while i < drafts.len() {
        if !too_short(&drafts[i], rules) {
            i += 1;
            continue;
        }
        if share(drafts, i, rules, true) {
            continue;
        }
        grow(drafts, i, rules);
        if too_short(&drafts[i], rules) && share(drafts, i, rules, false) {
            continue;
        }
        i += 1;
    }
}

fn too_short(d: &Draft, rules: &FrameRules) -> bool {
    d.kind == CueKind::Dialogue && d.end.saturating_sub(d.start) < rules.min
}

/// Merge cue `i` into the cue before it, else the one after it; `false` when neither fits.
fn share(drafts: &mut Vec<Draft>, i: usize, rules: &FrameRules, within_speed: bool) -> bool {
    if i > 0
        && let Some(merged) = merge(&drafts[i - 1], &drafts[i], rules, within_speed)
    {
        drafts[i - 1] = merged;
        drafts.remove(i);
        return true;
    }
    if let Some(next) = drafts.get(i + 1)
        && let Some(merged) = merge(&drafts[i], next, rules, within_speed)
    {
        drafts[i] = merged;
        drafts.remove(i + 1);
        return true;
    }
    false
}

/// `a` and the cue after it, `b`, as one cue: dashed when `b` starts a new speaker, else the words
/// broken again; `None` when they are not both plain dialogue close together, or the result does
/// not fit two lines, 7 s, or (when asked) 20 characters per second.
pub fn merge(a: &Draft, b: &Draft, rules: &FrameRules, within_speed: bool) -> Option<Draft> {
    let dialogue = |d: &Draft| d.kind == CueKind::Dialogue && !is_paired(d);
    if !dialogue(a) || !dialogue(b) || b.start.saturating_sub(a.end) > rules.shot_window {
        return None;
    }
    let italic = |d: &Draft| d.lines.iter().any(|l| l.italic);
    if italic(a) != italic(b) || b.end.saturating_sub(a.start) > rules.max {
        return None;
    }
    let lines = if b.starts_speaker {
        let one_line = |d: &Draft| d.lines.len() == 1 && d.lines[0].chars() < MAX_LINE;
        if italic(a) || !one_line(a) || !one_line(b) {
            return None;
        }
        vec![
            CueLine::plain(format!("-{}", a.lines[0].text)),
            CueLine::plain(format!("-{}", b.lines[0].text)),
        ]
    } else {
        let text: Vec<&str> = a
            .lines
            .iter()
            .chain(&b.lines)
            .flat_map(|l| l.text.split_whitespace())
            .collect();
        line_break::layout(&text)?
            .into_iter()
            .map(|l| {
                if italic(a) {
                    CueLine::italic(l)
                } else {
                    CueLine::plain(l)
                }
            })
            .collect()
    };
    let chars: usize = lines.iter().map(CueLine::chars).sum();
    let seconds = b.end.saturating_sub(a.start) as f64 * rules.rate.frame_s();
    if within_speed && (seconds <= 0.0 || chars as f64 / seconds > MAX_CPS) {
        return None;
    }
    Some(Draft {
        lines,
        kind: CueKind::Dialogue,
        speech_start_s: a.speech_start_s,
        speech_end_s: b.speech_end_s,
        start: a.start,
        end: b.end,
        starts_speaker: a.starts_speaker,
    })
}

/// A two-speaker cue already.
fn is_paired(d: &Draft) -> bool {
    d.lines.len() == 2 && d.lines.iter().all(|l| l.text.starts_with('-'))
}

/// Lengthen cue `i` toward the minimum: into the gap after it, then before it, where the cue
/// before gives back lead-out down to its speech and its own minimum.
fn grow(drafts: &mut [Draft], i: usize, rules: &FrameRules) {
    let limit = drafts
        .get(i + 1)
        .map_or(rules.total, |n| n.start.saturating_sub(rules.gap));
    let d = &mut drafts[i];
    let short = rules.min.saturating_sub(d.end - d.start);
    d.end += short.min(limit.saturating_sub(d.end));
    let short = rules.min.saturating_sub(drafts[i].end - drafts[i].start);
    if short == 0 {
        return;
    }
    let (onset, _) = drafts[i].speech_frames(rules.rate);
    let earliest = onset.saturating_sub(rules.lead_out);
    let wanted = drafts[i].start.saturating_sub(short).max(earliest);
    let start = if i == 0 {
        wanted
    } else {
        let prev = &mut drafts[i - 1];
        let (_, prev_done) = prev.speech_frames(rules.rate);
        let floor = prev_done.max(prev.start + rules.min);
        prev.end = prev.end.min(wanted.saturating_sub(rules.gap).max(floor));
        prev.end + rules.gap
    };
    let d = &mut drafts[i];
    d.start = start.max(wanted).min(d.start);
}

#[cfg(test)]
#[path = "tests/short.rs"]
mod tests;
