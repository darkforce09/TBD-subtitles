//! Frame scores to speech regions: threshold, pad, merge.

use job_model::outputs::TimeSpan;

/// Frames at or above `threshold` become regions, each padded by `pad_s` on both sides (within
/// `0..duration_s`), and regions closer than `merge_gap_s` are joined.
pub fn from_scores(
    scores: &[f32],
    frame_s: f64,
    threshold: f32,
    pad_s: f64,
    merge_gap_s: f64,
    duration_s: f64,
) -> Vec<TimeSpan> {
    let mut raw = Vec::new();
    let mut open: Option<usize> = None;
    for (i, score) in scores.iter().enumerate() {
        match (open, *score >= threshold) {
            (None, true) => open = Some(i),
            (Some(start), false) => {
                raw.push((start, i));
                open = None;
            }
            _ => {}
        }
    }
    if let Some(start) = open {
        raw.push((start, scores.len()));
    }
    let mut regions: Vec<TimeSpan> = Vec::new();
    for (start, end) in raw {
        let span = TimeSpan::new(
            (start as f64 * frame_s - pad_s).max(0.0),
            (end as f64 * frame_s + pad_s).min(duration_s),
        );
        match regions.last_mut() {
            Some(last) if span.start_s - last.end_s < merge_gap_s => last.end_s = span.end_s,
            _ => regions.push(span),
        }
    }
    regions
}

#[cfg(test)]
#[path = "tests/regions.rs"]
mod tests;
