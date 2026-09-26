//! Segmentation: each utterance cut into units that fit one cue, and neighbouring units shared
//! into one cue where a cue alone would be too short or too fast.
//!
//! **Role:** split each speaker's turn (the `||` of the final text) at sentence ends first, then
//! clauses, then pauses of 250 ms or more, until every unit fits two lines of 42 characters and
//! 6.4 s of speech. A unit starts a new speaker at `||` or when its utterance is flagged `SPK`.
//! Where a unit, or the one after it, has too little room for the minimum duration or 20
//! characters per second, the two share a cue: as `-Line` / `-Line` when the change of speaker
//! was marked (each one line and one sentence, under 12 frames apart), as one cue of one speaker
//! when it was not and the words fit.
//!
//! **Position:** called by `mod.rs` first; `line_break.rs` decides what fits.
//!
//! **Signals and state:** none; pure.
//!
//! **Invariants:** every aligned word lands in exactly one draft, in order; narration is italic
//! and never paired; a paired cue has exactly two lines, one per speaker; two speakers never
//! share a cue without the dashes.

use job_model::outputs::{Aligned, AlignedWord};
use subtitle_formats::cue::{CueKind, CueLine};

use super::line_break::{self, MAX_LINE};
use super::{Draft, FrameRules};

/// Speech a unit may hold, leaving room for the lead-in and lead-out within 7 s.
pub const MAX_SPEECH_S: f64 = 6.4;
/// A pause this long is a place to split.
pub const PAUSE_S: f64 = 0.25;
/// Reading speed a cue should keep to.
pub const MAX_CPS: f64 = 20.0;

/// One speaker's words for one cue.
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub words: Vec<AlignedWord>,
    pub narrator: bool,
    /// The unit starts where the language model marked another speaker.
    pub speaker_change: bool,
}

impl Unit {
    pub fn start_s(&self) -> f64 {
        self.words.first().map_or(0.0, |w| w.start_s)
    }

    pub fn end_s(&self) -> f64 {
        self.words.last().map_or(0.0, |w| w.end_s)
    }

    fn texts(&self) -> Vec<&str> {
        self.words.iter().map(|w| w.text.as_str()).collect()
    }

    fn text(&self) -> String {
        self.texts().join(" ")
    }

    /// Sentence ends inside the unit, counting a last word without one as a sentence.
    fn sentences(&self) -> usize {
        let ends = self.words.iter().filter(|w| ends_sentence(&w.text)).count();
        ends + usize::from(self.words.last().is_some_and(|w| !ends_sentence(&w.text)))
    }
}

/// Every utterance's turns cut into units, in order.
pub fn units(aligned: &Aligned) -> Vec<Unit> {
    let mut out = Vec::new();
    for u in &aligned.utterances {
        let mut edges = vec![0];
        edges.extend(
            u.speaker_starts
                .iter()
                .copied()
                .filter(|&s| s > 0 && s < u.words.len()),
        );
        edges.push(u.words.len());
        edges.dedup();
        for (turn, pair) in edges.windows(2).enumerate() {
            for (piece, words) in split_turn(&u.words[pair[0]..pair[1]])
                .into_iter()
                .enumerate()
            {
                out.push(Unit {
                    words,
                    narrator: u.narrator,
                    speaker_change: (turn > 0 || u.new_speaker) && piece == 0,
                });
            }
        }
    }
    out
}

/// Cut one speaker's words until each piece fits.
pub fn split_turn(words: &[AlignedWord]) -> Vec<Vec<AlignedWord>> {
    let mut out = Vec::new();
    let mut rest = words;
    while !rest.is_empty() {
        if fits(rest) {
            out.push(rest.to_vec());
            break;
        }
        let mut best: Option<(u8, usize)> = None;
        for k in 1..rest.len() {
            if !fits(&rest[..k]) {
                break;
            }
            let class = boundary_class(&rest[k - 1], &rest[k]);
            if best.is_none_or(|(c, _)| class <= c) {
                best = Some((class, k));
            }
        }
        let k = best.map_or(1, |(_, k)| k);
        out.push(rest[..k].to_vec());
        rest = &rest[k..];
    }
    out
}

/// Lower is a better place to split: a sentence end, a clause, a pause, anything else, and last
/// a word that belongs with the next.
fn boundary_class(before: &AlignedWord, after: &AlignedWord) -> u8 {
    if ends_sentence(&before.text) {
        0
    } else if before.text.ends_with([',', ';', ':', '—']) || before.text.ends_with("--") {
        1
    } else if after.start_s - before.end_s >= PAUSE_S {
        2
    } else if line_break::weak_end(&before.text) {
        4
    } else {
        3
    }
}

fn fits(words: &[AlignedWord]) -> bool {
    let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
    let span = words.last().map_or(0.0, |w| w.end_s) - words.first().map_or(0.0, |w| w.start_s);
    line_break::fits(&texts) && (span <= MAX_SPEECH_S || words.len() == 1)
}

fn ends_sentence(word: &str) -> bool {
    word.trim_end_matches(['"', '\'', ')', '’', '”'])
        .ends_with(['.', '!', '?', '…'])
}

/// The units laid out as drafts. Where a unit alone would be too short or too fast for its room,
/// it shares a cue with its neighbour: as `-Line` / `-Line` when the language model marked a
/// speaker change between them, as one cue when it did not.
pub fn drafts(units: &[Unit], rules: &FrameRules) -> Vec<Draft> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < units.len() {
        let unit = &units[i];
        if let Some(next) = units.get(i + 1) {
            let after = units.get(i + 2).map(Unit::start_s);
            let cramped = cramped(unit, Some(next.start_s()), rules) || cramped(next, after, rules);
            if cramped && pairs(unit, next, rules) {
                let mut paired = Draft::new(
                    vec![
                        CueLine::plain(format!("-{}", unit.text())),
                        CueLine::plain(format!("-{}", next.text())),
                    ],
                    CueKind::Dialogue,
                    unit.start_s(),
                    next.end_s(),
                );
                paired.starts_speaker = unit.speaker_change;
                out.push(paired);
                i += 2;
                continue;
            }
            if cramped && joins(unit, next, rules) {
                let joined = Unit {
                    words: [unit.words.clone(), next.words.clone()].concat(),
                    narrator: unit.narrator,
                    speaker_change: unit.speaker_change,
                };
                out.push(laid_out(&joined));
                i += 2;
                continue;
            }
        }
        out.push(laid_out(unit));
        i += 1;
    }
    out
}

/// One speaker's unit as a draft, italic for the narrator.
fn laid_out(unit: &Unit) -> Draft {
    let lines = line_break::layout(&unit.texts()).unwrap_or_else(|| vec![unit.text()]);
    let lines = lines
        .into_iter()
        .map(|l| {
            if unit.narrator {
                CueLine::italic(l)
            } else {
                CueLine::plain(l)
            }
        })
        .collect();
    let mut draft = Draft::new(lines, CueKind::Dialogue, unit.start_s(), unit.end_s());
    draft.starts_speaker = unit.speaker_change;
    draft
}

/// Whether `u` alone, from its lead-in to two frames before `next_start`, is shorter than the
/// minimum or reads faster than `MAX_CPS`; never when nothing follows it.
fn cramped(u: &Unit, next_start: Option<f64>, rules: &FrameRules) -> bool {
    let Some(next_start) = next_start else {
        return false;
    };
    let rate = rules.rate;
    let start = rate.frame_floor(u.start_s()).saturating_sub(rules.lead_in);
    let room = rate
        .frame_floor(next_start)
        .saturating_sub(rules.lead_in + rules.gap)
        .saturating_sub(start);
    let seconds = room as f64 * rate.frame_s();
    room < rules.min || seconds <= 0.0 || u.text().chars().count() as f64 / seconds > MAX_CPS
}

/// Whether `a` and the next unit `b` may share a two-speaker cue: a marked change, no narrator,
/// one line and one sentence each, less than `shot_window` frames apart.
fn pairs(a: &Unit, b: &Unit, rules: &FrameRules) -> bool {
    if !b.speaker_change || a.narrator || b.narrator {
        return false;
    }
    let one_line = |u: &Unit| u.text().chars().count() < MAX_LINE;
    one_line(a) && one_line(b) && a.sentences() <= 1 && b.sentences() <= 1 && close(a, b, rules)
}

/// Whether `a` and the next unit `b`, one speaker, fit one cue: no marked change, the same voice
/// (narrator or not), close together, two lines and the speech limit.
fn joins(a: &Unit, b: &Unit, rules: &FrameRules) -> bool {
    if b.speaker_change || a.narrator != b.narrator || !close(a, b, rules) {
        return false;
    }
    let words: Vec<AlignedWord> = [a.words.clone(), b.words.clone()].concat();
    fits(&words)
}

fn close(a: &Unit, b: &Unit, rules: &FrameRules) -> bool {
    let rate = rules.rate;
    rate.frame_floor(b.start_s())
        .saturating_sub(rate.frame_ceil(a.end_s()))
        < rules.shot_window
}

#[cfg(test)]
#[path = "tests/segment.rs"]
mod tests;
