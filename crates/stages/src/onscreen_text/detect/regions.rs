//! Association of screened text regions into occurrences.
//!
//! **Role:** decide which observation continues which active region, start and extend
//! occurrences, and compare region pictures through bounded grey signatures.
//! **Position:** used by the scan coordinator for every sample and by the bisection probes.
//! **Signals and state:** the active regions with their fixed anchor boxes and signatures; no I/O.
//! **Invariants:** a match is unique in both directions; an anchor box and its signature never
//! change after the region starts, and every later picture is compared at that same box, so a
//! detector box that jitters around static writing never splits it; an occurrence's frames tile,
//! each ending where the next begins; the limits fail explicitly instead of discarding text.

use std::path::PathBuf;

use image::{GrayImage, imageops};
use job_model::onscreen::{
    Quad, TextDocument, TextFrame, TextOccurrence, TextPresentation, TextProvenance,
};
use job_model::outputs::ShotChanges;

use crate::onscreen_text::TextResult;

pub(crate) const OBSERVATION_LIMIT: usize = 1_000_000;
pub(crate) const OCCURRENCE_LIMIT: usize = 100_000;
/// The box overlap above which two quads show the same region.
pub(crate) const SAME_REGION: f64 = 0.45;
/// Writing shown for less than this is one animation drawing of screening noise, not a sign.
pub(crate) const MIN_OCCURRENCE_S: f64 = 0.15;
/// A region covering more of the frame than this share is a merged false detection.
pub(crate) const MAX_REGION_SHARE: f64 = 0.5;

/// A region followed from sample to sample: its latest box in source pixels, and the fixed
/// anchor box and picture signature of its first observation.
pub(crate) struct Active {
    pub(crate) occurrence: usize,
    pub(crate) quad: Quad,
    /// The signature of the region's first observation; later pictures never replace it.
    pub(crate) anchor: GrayImage,
    /// Where the anchor was taken; every later comparison crops this same box.
    pub(crate) anchor_box: Quad,
}

/// One detected region on one frame: its box in source pixels.
pub(crate) struct Observation {
    pub(crate) quad: Quad,
    pub(crate) confidence: f64,
    pub(crate) surface_rgb: Option<[u8; 3]>,
}

pub(crate) fn check_limits(
    observations: usize,
    occurrences: usize,
    creating: bool,
) -> TextResult<()> {
    if observations > OBSERVATION_LIMIT || (creating && occurrences >= OCCURRENCE_LIMIT) {
        return Err("The visual scan reached its bounded observation limit. Split this unusually dense video into shorter jobs; no text is silently discarded.".into());
    }
    Ok(())
}

/// Whether an occurrence lasts long enough and covers little enough of the frame to be writing
/// rather than one drawing's worth of screening noise or a merged false detection.
pub(crate) fn plausible(item: &TextOccurrence, frame_area: f64) -> bool {
    let (left, top, right, bottom) = item
        .frames
        .first()
        .map(|frame| frame.quad.bounds())
        .unwrap_or((0.0, 0.0, 0.0, 0.0));
    let share = ((right - left) * (bottom - top)) / frame_area.max(1.0);
    item.end_s - item.start_s >= MIN_OCCURRENCE_S - 1e-9 && share <= MAX_REGION_SHARE
}

pub(crate) fn crosses_cut(cuts: &ShotChanges, previous: f64, current: f64) -> bool {
    cuts.cuts
        .iter()
        .any(|cut| cut.time_s > previous && cut.time_s <= current)
}

pub(crate) fn start_occurrence(document: &mut TextDocument, time: f64, confidence: f64) -> usize {
    let index = document.occurrences.len();
    let id = format!("text-{:06}", index + 1);
    document.occurrences.push(TextOccurrence {
        source_fingerprint: None,
        crops: vec![PathBuf::from(format!("visual/crops/{id}.png"))],
        id,
        start_s: time,
        end_s: time,
        japanese: String::new(),
        english: None,
        confidence,
        frames: Vec::new(),
        provenance: TextProvenance::default(),
        presentation: TextPresentation::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: None,
        keyframe: None,
        ruby: Vec::new(),
    });
    index
}

/// Extends an occurrence by an observation from `start` to `end`; the previous frame now ends
/// at `start`, so the frames tile.
pub(crate) fn append_observation(
    item: &mut TextOccurrence,
    observation: &Observation,
    start: f64,
    end: f64,
) {
    if let Some(last) = item.frames.last_mut() {
        last.end_s = start;
    }
    item.end_s = end;
    item.frames.push(TextFrame {
        time_s: start,
        end_s: end,
        quad: observation.quad,
        confidence: observation.confidence,
        surface_rgb: observation.surface_rgb,
    });
}

/// Matches observations to active regions: the boxes overlap and the current picture at the
/// region's anchor box still shows its anchor (`unchanged`, one flag per active region). Two-way
/// uniqueness prevents order-dependent swaps between overlapping text regions.
pub(crate) fn associate(
    active: &[Active],
    current: &[Observation],
    unchanged: &[bool],
) -> Vec<Option<usize>> {
    let mut matches = Vec::with_capacity(current.len());
    let mut uses = vec![0usize; active.len()];
    for observation in current {
        let mut candidates = Vec::new();
        for (index, prior) in active.iter().enumerate() {
            if overlap(prior.quad, observation.quad) > SAME_REGION
                && unchanged.get(index).copied().unwrap_or(false)
            {
                candidates.push(index);
                uses[index] += 1;
            }
        }
        matches.push(if candidates.len() == 1 {
            candidates.first().copied()
        } else {
            None
        });
    }
    matches
        .into_iter()
        .map(|candidate| candidate.filter(|&index| uses[index] == 1))
        .collect()
}

/// A bounded grey thumbnail: the short side at most 48 pixels, the long side at most 768,
/// never upscaled.
pub(crate) fn signature(image: &GrayImage) -> GrayImage {
    let (w, h) = image.dimensions();
    let scale = (48.0 / f64::from(w.min(h)))
        .min(768.0 / f64::from(w.max(h)))
        .min(1.0);
    imageops::resize(
        image,
        (f64::from(w) * scale).round().max(1.0) as u32,
        (f64::from(h) * scale).round().max(1.0) as u32,
        imageops::FilterType::Triangle,
    )
}

/// Local limits preserve a changed glyph even when it occupies little of a long line.
pub(crate) fn same_signature(anchor: &GrayImage, current: &GrayImage) -> bool {
    if anchor == current {
        return true;
    }
    let ratio = f64::from(anchor.width()) / f64::from(current.width());
    let height_ratio = f64::from(anchor.height()) / f64::from(current.height());
    if !(0.9..=1.1).contains(&ratio)
        || !(0.9..=1.1).contains(&height_ratio)
        || !(0.95..=1.05).contains(&(ratio / height_ratio))
    {
        return false;
    }
    let resized;
    let current = if anchor.dimensions() == current.dimensions() {
        current
    } else {
        resized = imageops::resize(
            current,
            anchor.width(),
            anchor.height(),
            imageops::FilterType::Triangle,
        );
        &resized
    };
    let cost = |dx, dy| {
        (0..anchor.height())
            .step_by(4)
            .flat_map(|y| {
                (0..anchor.width())
                    .step_by(4)
                    .map(move |x| u64::from(delta(anchor, current, x, y, dx, dy)))
            })
            .sum::<u64>()
    };
    let (mut shift, mut best) = ((0, 0), cost(0, 0));
    for dy in -2..=2 {
        for dx in -2..=2 {
            let score = cost(dx, dy);
            if score < best {
                best = score;
                shift = (dx, dy);
            }
        }
    }
    let mut total = 0u64;
    for top in (0..anchor.height()).step_by(8) {
        for left in (0..anchor.width()).step_by(8) {
            let mut local = 0u64;
            let mut count = 0u64;
            for y in top..(top + 8).min(anchor.height()) {
                for x in left..(left + 8).min(anchor.width()) {
                    let difference = delta(anchor, current, x, y, shift.0, shift.1);
                    if difference > 180 {
                        return false;
                    }
                    local += u64::from(difference);
                    count += 1;
                }
            }
            if local > count * 6 {
                return false;
            }
            total += local;
        }
    }
    total <= u64::from(anchor.width()) * u64::from(anchor.height()) * 4
}

fn delta(a: &GrayImage, b: &GrayImage, x: u32, y: u32, dx: i64, dy: i64) -> u8 {
    let bx = (i64::from(x) + dx).clamp(0, i64::from(b.width()) - 1) as u32;
    let by = (i64::from(y) + dy).clamp(0, i64::from(b.height()) - 1) as u32;
    a.get_pixel(x, y).0[0].abs_diff(b.get_pixel(bx, by).0[0])
}

/// Intersection over union of the two quads' bounding boxes.
pub(crate) fn overlap(a: Quad, b: Quad) -> f64 {
    let (al, at, ar, ab) = a.bounds();
    let (bl, bt, br, bb) = b.bounds();
    let intersection = (ar.min(br) - al.max(bl)).max(0.0) * (ab.min(bb) - at.max(bt)).max(0.0);
    intersection / ((ar - al) * (ab - at) + (br - bl) * (bb - bt) - intersection).max(1.0)
}

#[cfg(test)]
#[path = "tests/regions.rs"]
mod tests;
