//! Which frames a localized video re-encodes and which it copies from the source.
//!
//! **Role:** widen each run of changed frames to the IDR keyframes around it, merge runs that
//! meet, and lay the video out as an ordered list of pieces, each copied from the source or
//! re-encoded, that covers every frame exactly once.
//! **Position:** inside `encode::segments`; the localize stage plans with the source's keyframes
//! and an IDR check (`IdrProbe::confirm` in practice, a fake in tests).
//! **Signals and state:** pure apart from the IDR check it is handed, which it asks in batches,
//! never twice about one frame.
//! **Invariants:** every re-encoded piece starts at frame 0 or at a confirmed IDR keyframe, and
//! every copied piece starts at a confirmed IDR keyframe, so each piece decodes without any frame
//! before it; a keyframe that is not an IDR widens the run to the next IDR outward; the pieces are
//! in order, never empty, and cover `0..frame_count` with no gap or overlap; two re-encoded pieces
//! never touch.

use std::collections::{BTreeSet, HashMap};
use std::fmt;

use job_model::onscreen::SegmentSummary;

use crate::MediaError;

/// A run of frames by presentation index, both ends included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSpan {
    pub first: u64,
    pub last: u64,
}

/// One piece of the localized video, frames by presentation index with both ends included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece {
    /// The source's packets for these frames, copied unchanged.
    Copy { first: u64, last: u64 },
    /// These frames decoded, changed and encoded again.
    Encode { first: u64, last: u64 },
}

impl Piece {
    pub fn first(self) -> u64 {
        match self {
            Piece::Copy { first, .. } | Piece::Encode { first, .. } => first,
        }
    }

    pub fn last(self) -> u64 {
        match self {
            Piece::Copy { last, .. } | Piece::Encode { last, .. } => last,
        }
    }

    /// How many frames the piece holds.
    pub fn frames(self) -> u64 {
        self.last() - self.first() + 1
    }

    pub fn is_copy(self) -> bool {
        matches!(self, Piece::Copy { .. })
    }
}

/// Why no plan was made.
#[derive(Debug)]
pub enum PlanError {
    /// The changed frames or the keyframes do not fit the video.
    Invalid(String),
    /// The IDR check failed to run or answered the wrong number of frames.
    Probe(MediaError),
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::Invalid(message) => write!(f, "invalid segment plan: {message}"),
            PlanError::Probe(error) => write!(f, "the IDR check failed: {error}"),
        }
    }
}

impl std::error::Error for PlanError {}

/// The IDR check a plan asks: for a batch of keyframes, ascending, which open an IDR picture.
pub type IdrCheck<'a> = &'a mut dyn FnMut(&[u64]) -> Result<Vec<bool>, MediaError>;

/// The pieces of a `frame_count`-frame video whose `changed` frames are re-encoded.
///
/// `keyframes` are the presentation indices the container flags as keyframes, ascending.
/// `confirm` answers, for a batch of keyframes, which open an IDR picture. A run of changed frames
/// starts its piece at the last IDR keyframe at or before it (frame 0 needs no check) and ends it
/// just before the first IDR keyframe after it (or at the last frame); runs whose pieces meet
/// become one piece. No changed frames make one copied piece.
pub fn plan_pieces(
    changed: &[FrameSpan],
    keyframes: &[u64],
    frame_count: u64,
    confirm: IdrCheck,
) -> Result<Vec<Piece>, PlanError> {
    validate(changed, keyframes, frame_count)?;
    let runs = merge_spans(changed);
    let mut bounds: Vec<Bound> = runs
        .iter()
        .map(|run| Bound::around(*run, keyframes))
        .collect();
    let mut known: HashMap<u64, bool> = HashMap::new();
    loop {
        let mut needed = BTreeSet::new();
        for bound in &mut bounds {
            bound.settle(keyframes, frame_count, &known, &mut needed);
        }
        if needed.is_empty() {
            break;
        }
        let frames: Vec<u64> = needed.into_iter().collect();
        let answers = confirm(&frames).map_err(PlanError::Probe)?;
        if answers.len() != frames.len() {
            return Err(PlanError::Probe(MediaError::Parse(format!(
                "the IDR check answered {} of {} keyframes",
                answers.len(),
                frames.len()
            ))));
        }
        known.extend(frames.into_iter().zip(answers));
    }
    let widened = bounds
        .iter()
        .filter_map(|bound| Some((bound.start?, bound.end?)));
    Ok(lay_out(widened, frame_count))
}

/// How much of the video `pieces` re-encode and copy, with no fallback reason.
pub fn piece_summary(pieces: &[Piece]) -> SegmentSummary {
    let (encoded, copied): (Vec<Piece>, Vec<Piece>) =
        pieces.iter().partition(|piece| !piece.is_copy());
    SegmentSummary {
        segments_reencoded: encoded.len(),
        frames_reencoded: encoded.iter().map(|piece| piece.frames()).sum(),
        frames_copied: copied.iter().map(|piece| piece.frames()).sum(),
        fallback_reason: None,
    }
}

/// Where one run's re-encoded piece starts and ends (exclusive), and the keyframes still being
/// tried for either end.
struct Bound {
    /// The position in the keyframe list of the start being tried.
    start_try: Option<usize>,
    start: Option<u64>,
    /// The position in the keyframe list of the end being tried.
    end_try: usize,
    end: Option<u64>,
}

impl Bound {
    fn around(run: FrameSpan, keyframes: &[u64]) -> Bound {
        let at_or_before = keyframes.partition_point(|&key| key <= run.first);
        Bound {
            start_try: at_or_before.checked_sub(1),
            start: None,
            end_try: keyframes.partition_point(|&key| key <= run.last),
            end: None,
        }
    }

    /// Settle each end the known answers allow, moving outward past keyframes that are not IDR,
    /// and add the keyframe still to be asked about, if any, to `needed`.
    fn settle(
        &mut self,
        keyframes: &[u64],
        frame_count: u64,
        known: &HashMap<u64, bool>,
        needed: &mut BTreeSet<u64>,
    ) {
        while self.start.is_none() {
            let Some(position) = self.start_try else {
                self.start = Some(0);
                break;
            };
            let key = keyframes[position];
            match (key, known.get(&key)) {
                (0, _) | (_, Some(true)) => self.start = Some(key),
                (_, Some(false)) => self.start_try = position.checked_sub(1),
                (_, None) => {
                    needed.insert(key);
                    break;
                }
            }
        }
        while self.end.is_none() {
            let Some(&key) = keyframes.get(self.end_try) else {
                self.end = Some(frame_count);
                break;
            };
            match known.get(&key) {
                Some(true) => self.end = Some(key),
                Some(false) => self.end_try += 1,
                None => {
                    needed.insert(key);
                    break;
                }
            }
        }
    }
}

/// The ordered pieces for re-encoded ranges `[start, end)`, ascending by start: ranges that
/// overlap or meet become one, and the gaps between them are copied.
fn lay_out(widened: impl Iterator<Item = (u64, u64)>, frame_count: u64) -> Vec<Piece> {
    let mut merged: Vec<(u64, u64)> = Vec::new();
    for (start, end) in widened {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    let mut pieces = Vec::new();
    let mut next = 0;
    for (start, end) in merged {
        if start > next {
            pieces.push(Piece::Copy {
                first: next,
                last: start - 1,
            });
        }
        pieces.push(Piece::Encode {
            first: start,
            last: end - 1,
        });
        next = end;
    }
    if next < frame_count {
        pieces.push(Piece::Copy {
            first: next,
            last: frame_count - 1,
        });
    }
    pieces
}

/// The changed spans sorted, with spans that overlap or meet joined.
fn merge_spans(changed: &[FrameSpan]) -> Vec<FrameSpan> {
    let mut spans = changed.to_vec();
    spans.sort_by_key(|span| span.first);
    let mut merged: Vec<FrameSpan> = Vec::new();
    for span in spans {
        match merged.last_mut() {
            Some(last) if span.first <= last.last.saturating_add(1) => {
                last.last = last.last.max(span.last)
            }
            _ => merged.push(span),
        }
    }
    merged
}

fn validate(changed: &[FrameSpan], keyframes: &[u64], frame_count: u64) -> Result<(), PlanError> {
    let invalid = |message: String| Err(PlanError::Invalid(message));
    if frame_count == 0 {
        return invalid("the video has no frames".into());
    }
    if let Some(span) = changed
        .iter()
        .find(|span| span.first > span.last || span.last >= frame_count)
    {
        return invalid(format!(
            "frames {}..={} do not lie within the {frame_count} frames",
            span.first, span.last
        ));
    }
    if keyframes.windows(2).any(|pair| pair[0] >= pair[1])
        || keyframes.last().is_some_and(|&last| last >= frame_count)
    {
        return invalid("the keyframes are not ascending frames of the video".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/plan.rs"]
mod tests;
