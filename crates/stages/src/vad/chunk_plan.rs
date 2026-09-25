//! Speech regions to chunks: 20 to 60 seconds, cut in silences, never across a long pause.
//!
//! **Role:** decide where the audio is cut for the speech engines, so every engine transcribes the
//! same chunks and their words line up.
//!
//! **Position:** called by `mod.rs` (`plan`); pure, on `job_model` time spans.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** chunks are ordered, disjoint and at most `max_s` long; a cut falls inside
//! speech only when one region alone is longer than `max_s`.

use job_model::outputs::TimeSpan;

/// How chunks are cut.
#[derive(Debug, Clone, Copy)]
pub struct ChunkSettings {
    /// A chunk grows past its regions until at least this long, when the speech allows.
    pub min_s: f64,
    /// No chunk is longer.
    pub max_s: f64,
    /// The shortest silence a chunk may end in.
    pub min_silence_s: f64,
    /// A silence this long always ends a chunk, so no chunk carries a long stretch of music.
    pub break_s: f64,
}

impl Default for ChunkSettings {
    fn default() -> ChunkSettings {
        ChunkSettings {
            min_s: 20.0,
            max_s: 60.0,
            min_silence_s: 0.35,
            break_s: 3.0,
        }
    }
}

/// Cut `regions` into chunks. A chunk starts at a region's start and ends at a region's end, so
/// every cut lies in a silence; a single region longer than `max_s` is split at its quietest
/// frames (`scores`, one per `frame_s`), the one place a cut may fall inside speech.
pub fn cut(
    regions: &[TimeSpan],
    scores: &[f32],
    frame_s: f64,
    settings: &ChunkSettings,
) -> Vec<TimeSpan> {
    let mut chunks: Vec<TimeSpan> = Vec::new();
    let mut current: Option<TimeSpan> = None;
    for region in regions {
        for piece in split_long(*region, scores, frame_s, settings.max_s) {
            current = match current {
                None => Some(piece),
                Some(chunk) => {
                    let gap = piece.start_s - chunk.end_s;
                    let grown = piece.end_s - chunk.start_s;
                    let may_cut = gap >= settings.min_silence_s;
                    let must_cut = grown > settings.max_s || gap >= settings.break_s;
                    let wants_cut = chunk.duration_s() >= settings.min_s;
                    if may_cut && (must_cut || wants_cut) || grown > settings.max_s {
                        chunks.push(chunk);
                        Some(piece)
                    } else {
                        Some(TimeSpan::new(chunk.start_s, piece.end_s))
                    }
                }
            };
        }
    }
    chunks.extend(current);
    chunks
}

/// Split a region longer than `max_s`: from each piece's start, cut at the lowest-scoring frame
/// between half and all of `max_s` further on (the latest one on a tie), until the rest fits.
fn split_long(region: TimeSpan, scores: &[f32], frame_s: f64, max_s: f64) -> Vec<TimeSpan> {
    let mut pieces = Vec::new();
    let mut start = region.start_s;
    while region.end_s - start > max_s {
        let first = ((start + max_s * 0.5) / frame_s) as usize;
        let last = ((start + max_s) / frame_s) as usize;
        let quietest = (first..=last)
            .filter(|&i| i < scores.len())
            .min_by(|&a, &b| scores[a].total_cmp(&scores[b]).then(b.cmp(&a)))
            .unwrap_or(last);
        let at = (quietest as f64 * frame_s).clamp(start + max_s * 0.5, start + max_s);
        pieces.push(TimeSpan::new(start, at));
        start = at;
    }
    pieces.push(TimeSpan::new(start, region.end_s));
    pieces
}

#[cfg(test)]
#[path = "tests/chunk_plan.rs"]
mod tests;
