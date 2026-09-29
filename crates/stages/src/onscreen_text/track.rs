//! Geometry verification of visible writing across its observed frames.
//!
//! **Role:** accept a static text surface when every observed quad keeps its centre and its
//! overlap with the keyframe's full-resolution quad, and send moving or unverified writing to
//! nearby placement.
//! **Position:** visual step between reading and translation; reads only the text document.
//! **Signals and state:** one pass over the occurrences; no decoding and no image buffers.
//! **Invariants:** timing is unchanged; confidence alone never verifies geometry; a detector box
//! that grows or shrinks around unmoved writing is not motion, a centre that drifts is; a
//! verified surface carries the keyframe quad on every frame; anything else is flagged for a
//! readable nearby translation.

use inference::ocr::OcrError;
use job_model::onscreen::{Quad, TextDocument, TextOccurrence, TextTreatment};
use job_model::outputs::VideoStream;

use super::geometry;

/// The least overlap an observed box keeps with the keyframe box on a static surface.
const MIN_OVERLAP: f64 = 0.5;
/// How far a box centre may drift, as a share of the keyframe box height, on a static surface.
const CENTRE_DRIFT: f64 = 0.2;

pub fn track(
    document: &mut TextDocument,
    stream: &VideoStream,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<(), OcrError> {
    if document.occurrences.is_empty() {
        progress(0, 0);
        return Ok(());
    }
    if document.width != stream.width || document.height != stream.height {
        return Err("Text tracking geometry does not match the source video dimensions".into());
    }
    let tolerance = tolerance(document);
    let total = document.occurrences.len();
    for (done, text) in document.occurrences.iter_mut().enumerate() {
        verify(text, tolerance);
        progress(done + 1, total);
    }
    Ok(())
}

/// The least centre drift always allowed: two pixels at 1080 rows, widened by the screening
/// copy's scale, since the other frames' quads come from the proxy.
fn tolerance(document: &TextDocument) -> f64 {
    let proxy_scale = if document.proxy_width == 0 {
        1.0
    } else {
        f64::from(document.width) / f64::from(document.proxy_width)
    };
    2.0 * f64::from(document.height) / 1080.0 * proxy_scale.max(1.0)
}

fn verify(text: &mut TextOccurrence, tolerance: f64) {
    let reference = text.keyframe.as_ref().and_then(|keyframe| {
        text.frames
            .iter()
            .find(|frame| (frame.time_s - keyframe.time_s).abs() <= 1e-6)
            .map(|frame| frame.quad)
    });
    let Some(reference) = reference else {
        return flag(text, "no keyframe geometry verifies the text surface");
    };
    if text
        .frames
        .iter()
        .all(|frame| unmoved(frame.quad, reference, tolerance))
    {
        for frame in &mut text.frames {
            frame.quad = reference;
        }
    } else {
        flag(text, "the writing moves between sampled frames");
    }
}

/// Whether an observed box shows the same unmoved surface as the reference: its centre stays
/// within `CENTRE_DRIFT` of the reference height (at least `tolerance`) and the boxes overlap by
/// `MIN_OVERLAP`, so a detector box that merely grows or shrinks around the writing passes.
fn unmoved(observed: Quad, reference: Quad, tolerance: f64) -> bool {
    let (left, top, right, bottom) = reference.bounds();
    let drift = geometry::distance(observed.center(), reference.center());
    drift <= (CENTRE_DRIFT * (bottom - top)).max(tolerance)
        && overlap(observed, reference) >= MIN_OVERLAP
        && right > left
}

/// Intersection over union of the boxes' axis-aligned bounds.
fn overlap(a: Quad, b: Quad) -> f64 {
    let (al, at, ar, ab) = a.bounds();
    let (bl, bt, br, bb) = b.bounds();
    let intersection = (ar.min(br) - al.max(bl)).max(0.0) * (ab.min(bb) - at.max(bt)).max(0.0);
    intersection / ((ar - al) * (ab - at) + (br - bl) * (bb - bt) - intersection).max(1.0)
}

fn flag(text: &mut TextOccurrence, reason: &str) {
    text.presentation.treatment = TextTreatment::Nearby;
    let warning = format!("Tracking needs review: {reason}. English uses nearby placement.");
    if !text.warnings.contains(&warning) {
        text.warnings.push(warning);
    }
}

#[cfg(test)]
#[path = "tests/track.rs"]
mod tests;
