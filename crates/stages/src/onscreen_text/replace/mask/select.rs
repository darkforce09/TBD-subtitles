//! Which occurrences are replaced, their frame spans and the rectangles measured around them.
//!
//! **Role:** map an occurrence's times onto the decoded frame timeline and derive its keyframe
//! quad, analysis window and plate rectangle.
//! **Position:** the first part of mask extraction, before any pixel is decoded.
//! **Signals and state:** pure functions of the text document and the frame timeline.
//! **Invariants:** a span covers exactly the frames that start inside the occurrence's interval;
//! every rectangle returned lies inside the frame and is non-empty.

use job_model::onscreen::{PixelRect, Quad, TextFrame, TextOccurrence, TextTreatment};

/// Tolerance for frame starts that sit on an interval boundary, in seconds.
const BOUNDARY_S: f64 = 1e-3;
/// Least confidence of unreviewed English that is shown at all.
const DISPLAYABLE_CONFIDENCE: f64 = 0.85;
/// Most a sampled quad corner may differ from the keyframe quad on static writing, in pixels.
const STATIC_TOLERANCE_PX: f64 = 0.5;
/// Pixels around the quad that segmentation examines.
pub(super) const ANALYSIS_MARGIN: u32 = 4;
/// Least inpainting context around the quad, in pixels.
const MIN_CONTEXT: f64 = 32.0;
/// Inpainting context around the quad as a share of its shorter side.
const CONTEXT_SHARE: f64 = 0.75;

/// Whether the occurrence's English is shown and could be drawn into the picture.
pub(super) fn candidate(text: &TextOccurrence) -> bool {
    text.english
        .as_deref()
        .is_some_and(|e| !e.trim().is_empty())
        && (text.reviewed
            || (text.confidence.is_finite() && text.confidence >= DISPLAYABLE_CONFIDENCE))
        && !text.frames.is_empty()
        && text.keyframe.is_some()
}

/// Whether the owner chose a nearby label instead of replacement. Earlier steps also choose
/// nearby placement for the subtitle file (moving writing, writing only Claude found); that
/// choice concerns ASS drawing, so replacement is still attempted for unreviewed occurrences.
pub(super) fn nearby(text: &TextOccurrence) -> bool {
    text.reviewed && text.presentation.treatment == TextTreatment::Nearby
}

/// The first and last frame starting inside `[start_s, end_s)`, or `None` when no frame does.
pub(super) fn span(timeline: &[(f64, f64)], start_s: f64, end_s: f64) -> Option<(u64, u64)> {
    let first = timeline.partition_point(|(s, _)| *s < start_s - BOUNDARY_S);
    let end = timeline.partition_point(|(s, _)| *s < end_s - BOUNDARY_S);
    (first < end).then(|| (first as u64, end as u64 - 1))
}

/// The frame showing `time_s`: the one whose interval contains it, else the nearest start.
/// `None` only for an empty timeline.
pub(super) fn frame_at(timeline: &[(f64, f64)], time_s: f64) -> Option<u64> {
    let after = timeline.partition_point(|(s, _)| *s <= time_s);
    if after > 0 && time_s < timeline[after - 1].1 {
        return Some(after as u64 - 1);
    }
    let before = after.checked_sub(1);
    let next = (after < timeline.len()).then_some(after);
    match (before, next) {
        (Some(b), Some(n)) => {
            let nearer_next = (timeline[n].0 - time_s).abs() < (time_s - timeline[b].0).abs();
            Some(if nearer_next { n } else { b } as u64)
        }
        (Some(b), None) => Some(b as u64),
        (None, Some(n)) => Some(n as u64),
        (None, None) => None,
    }
}

/// The observed quad at `time_s`: the frame whose interval contains it, else the nearest one.
pub(super) fn keyframe_quad(frames: &[TextFrame], time_s: f64) -> Option<Quad> {
    frames
        .iter()
        .find(|f| f.time_s <= time_s && time_s < f.end_s)
        .or_else(|| {
            frames.iter().min_by(|a, b| {
                gap(a, time_s)
                    .partial_cmp(&gap(b, time_s))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        })
        .map(|f| f.quad)
}

fn gap(frame: &TextFrame, time_s: f64) -> f64 {
    if time_s < frame.time_s {
        frame.time_s - time_s
    } else {
        (time_s - frame.end_s).max(0.0)
    }
}

/// Whether every observed quad sits on the keyframe quad, as tracking snaps static writing.
pub(super) fn is_static(frames: &[TextFrame], key: Quad) -> bool {
    frames.iter().all(|f| {
        f.quad.0.iter().zip(key.0.iter()).all(|(a, b)| {
            (a.x - b.x).abs() <= STATIC_TOLERANCE_PX && (a.y - b.y).abs() <= STATIC_TOLERANCE_PX
        })
    })
}

/// The whole pixels the quad touches, expanded by `margin` and clipped to the frame.
pub(super) fn around(quad: Quad, margin: f64, width: u32, height: u32) -> Option<PixelRect> {
    let (l, t, r, b) = quad.bounds();
    let left = (l - margin).floor().max(0.0);
    let top = (t - margin).floor().max(0.0);
    let right = (r + margin).ceil().min(f64::from(width));
    let bottom = (b + margin).ceil().min(f64::from(height));
    if !(right > left && bottom > top) {
        return None;
    }
    Some(PixelRect {
        x: left as u32,
        y: top as u32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    })
}

/// The rectangle between two corners clipped to the frame; `None` when nothing is left.
pub(super) fn clamp(
    left: i64,
    top: i64,
    right: i64,
    bottom: i64,
    frame: (u32, u32),
) -> Option<PixelRect> {
    let (l, t) = (left.max(0), top.max(0));
    let (r, b) = (
        right.min(i64::from(frame.0)),
        bottom.min(i64::from(frame.1)),
    );
    (r > l && b > t).then(|| PixelRect {
        x: l as u32,
        y: t as u32,
        width: (r - l) as u32,
        height: (b - t) as u32,
    })
}

/// The smallest rectangle holding both.
pub(super) fn bounding(a: PixelRect, b: PixelRect) -> PixelRect {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    PixelRect {
        x,
        y,
        width: a.right().max(b.right()) - x,
        height: a.bottom().max(b.bottom()) - y,
    }
}

/// The inpainting context kept around the quad on each side, in pixels.
pub(super) fn context_margin(quad: Quad) -> f64 {
    MIN_CONTEXT.max((CONTEXT_SHARE * shorter_side(quad)).ceil())
}

/// Height of one line of the writing: the quad's shorter side shared among its lines.
pub(super) fn line_height(quad: Quad, japanese: &str) -> f64 {
    let lines = japanese.lines().filter(|l| !l.trim().is_empty()).count();
    shorter_side(quad) / lines.max(1) as f64
}

/// The shorter side of the quad's bounding box.
pub(super) fn shorter_side(quad: Quad) -> f64 {
    let (l, t, r, b) = quad.bounds();
    (r - l).min(b - t)
}

#[cfg(test)]
#[path = "tests/select.rs"]
mod tests;
