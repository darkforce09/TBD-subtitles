//! The kept utterances and the alignment blocks they are grouped into.
//!
//! **Role:** turn the final lines into displayed words per utterance (speaker changes out of the
//! text and into indices; lyric and dropped lines left out), and group consecutive utterances
//! into blocks of about 20–60 s that start and end at pauses, with the audio span each block is
//! aligned over.
//!
//! **Position:** used by `run.rs`; the sheet and lines come from the adjudication steps.
//!
//! **Signals and state:** none; pure.
//!
//! **Invariants:** a block never spans a lyric or dropped utterance; blocks cover the kept
//! utterances in order, each exactly once; a block's audio span never reaches into a
//! neighbouring block's speech.

use std::collections::HashMap;
use std::ops::Range;

use job_model::outputs::{Line, TimeSpan, TimedWord, Utterance};

/// A block is not closed before it is this long, unless an edge is forced.
pub const MIN_BLOCK_S: f64 = 20.0;
/// A block is closed at the first pause that keeps it under this length.
pub const MAX_BLOCK_S: f64 = 60.0;
/// A block is closed at any gap when it would grow past this length.
pub const HARD_MAX_BLOCK_S: f64 = 90.0;
/// The shortest gap a block may end at.
pub const MIN_PAUSE_S: f64 = 0.35;
/// Audio heard on either side of the speech of a block or utterance.
pub const PAD_S: f64 = 0.3;

/// One utterance whose text reaches the subtitles.
#[derive(Debug, Clone, PartialEq)]
pub struct Kept {
    pub id: String,
    /// Its position in the sheet.
    pub sheet_index: usize,
    /// The recognition window: the backbone's first and last word times.
    pub start_s: f64,
    pub end_s: f64,
    /// The displayed words of the final text.
    pub words: Vec<String>,
    /// Indices into `words` where another speaker starts.
    pub speaker_starts: Vec<usize>,
    /// The language model marked it (`SPK`) as started by another speaker than the one before.
    pub new_speaker: bool,
    pub narrator: bool,
    pub unsure: bool,
    /// The backbone engine's words, for comparison and fallback.
    pub backbone: Vec<TimedWord>,
}

/// The utterances to align: every sheet utterance with a final line that is not lyric, not
/// dropped and not empty, in sheet order.
pub fn kept(sheet: &[Utterance], lines: &[Line]) -> Vec<Kept> {
    let by_id: HashMap<&str, &Line> = lines.iter().map(|l| (l.id.as_str(), l)).collect();
    let mut out = Vec::new();
    for (sheet_index, u) in sheet.iter().enumerate() {
        let Some(line) = by_id.get(u.id.as_str()) else {
            continue;
        };
        if line.has_flag("LYRIC") || line.has_flag("DROP") {
            continue;
        }
        let (words, speaker_starts) = displayed_words(&line.t);
        if words.is_empty() {
            continue;
        }
        out.push(Kept {
            id: u.id.clone(),
            sheet_index,
            start_s: u.start_s,
            end_s: u.end_s,
            words,
            speaker_starts,
            new_speaker: line.has_flag("SPK"),
            narrator: line.has_flag("NARR"),
            unsure: line.has_flag("UNSURE"),
            backbone: u.words.clone(),
        });
    }
    out
}

/// The words of a final text and the indices where `||` starts another speaker.
pub fn displayed_words(text: &str) -> (Vec<String>, Vec<usize>) {
    let mut words = Vec::new();
    let mut starts = Vec::new();
    for token in text.replace("||", " || ").split_whitespace() {
        if token == "||" {
            if !words.is_empty() && starts.last() != Some(&words.len()) {
                starts.push(words.len());
            }
        } else {
            words.push(token.to_string());
        }
    }
    starts.retain(|&s| s < words.len());
    (words, starts)
}

/// Group `kept` into blocks: a new block starts after a lyric or dropped utterance, or at a
/// pause of `MIN_PAUSE_S` once the block is `MIN_BLOCK_S` long or would pass `MAX_BLOCK_S`, or
/// at any gap before it would pass `HARD_MAX_BLOCK_S`.
pub fn plan_blocks(kept: &[Kept]) -> Vec<Range<usize>> {
    let mut blocks = Vec::new();
    let mut start = 0;
    for i in 1..kept.len() {
        let (prev, next) = (&kept[i - 1], &kept[i]);
        let forced = next.sheet_index != prev.sheet_index + 1;
        let gap = next.start_s - prev.end_s;
        let length = prev.end_s - kept[start].start_s;
        let with_next = next.end_s - kept[start].start_s;
        let pause = gap >= MIN_PAUSE_S && (length >= MIN_BLOCK_S || with_next > MAX_BLOCK_S);
        if forced || pause || (with_next > HARD_MAX_BLOCK_S && gap > 0.0) {
            blocks.push(start..i);
            start = i;
        }
    }
    if !kept.is_empty() {
        blocks.push(start..kept.len());
    }
    blocks
}

/// The audio span of kept utterances `range`: their speech padded by `PAD_S`, but never past
/// the middle of the gap to the neighbouring kept utterance, nor outside the video.
pub fn audio_span(kept: &[Kept], range: Range<usize>, duration_s: f64) -> TimeSpan {
    let first = &kept[range.start];
    let last = &kept[range.end - 1];
    let mut start = first.start_s - PAD_S;
    if let Some(prev) = range.start.checked_sub(1).map(|i| &kept[i]) {
        start = start.max((prev.end_s + first.start_s) / 2.0);
    }
    let mut end = last.end_s + PAD_S;
    if let Some(next) = kept.get(range.end) {
        end = end.min((last.end_s + next.start_s) / 2.0);
    }
    TimeSpan::new(start.max(0.0), end.min(duration_s).max(start.max(0.0)))
}

#[cfg(test)]
#[path = "tests/blocks.rs"]
mod tests;
