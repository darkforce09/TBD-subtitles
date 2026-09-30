//! Which frames of a baked replacement the read-back check looks at.
//!
//! **Role:** pick a few telling frames per occurrence: its ends and middle, the frames where
//! another replacement over the same place starts or ends, and the starts of its plates.
//! **Position:** the first half of `verify`; pure over frame spans and keyframe areas.
//! **Signals and state:** none.
//! **Invariants:** every frame lies in the occurrence's span; frames are distinct and ascending;
//! at most `MAX_SAMPLES`, taken in the order ends and middle, overlap edges, plate starts, the
//! plate starts spread evenly when they do not all fit.

use std::collections::BTreeSet;

/// The most frames checked per occurrence.
pub const MAX_SAMPLES: usize = 8;
/// Intersection over union of two keyframe areas above which replacements share a place.
pub const OVERLAP_IOU: f64 = 0.3;

/// One baked occurrence as sampling sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub first_frame: u64,
    pub last_frame: u64,
    /// The first frame of each plate, in order.
    pub plate_starts: Vec<u64>,
    /// Where it is lettered at the keyframe: left, top, right, bottom in source pixels.
    pub area: (f64, f64, f64, f64),
}

/// The frames to check of `target`, given every other baked occurrence `others`.
pub fn sample_frames(target: &Candidate, others: &[&Candidate]) -> Vec<u64> {
    let (first, last) = (target.first_frame, target.last_frame);
    let inside = |frame: u64| (first..=last).contains(&frame);
    let mut ordered = vec![first, first + (last - first) / 2, last];
    for other in others {
        if iou(target.area, other.area) < OVERLAP_IOU
            || other.first_frame > last.saturating_add(1)
            || other.last_frame.saturating_add(1) < first
        {
            continue;
        }
        let edges = [
            other.first_frame.saturating_sub(1),
            other.first_frame,
            other.last_frame,
            other.last_frame.saturating_add(1),
        ];
        ordered.extend(edges.into_iter().filter(|&frame| inside(frame)));
    }
    let mut chosen: Vec<u64> = Vec::with_capacity(MAX_SAMPLES);
    let take = |frame: u64, chosen: &mut Vec<u64>| {
        if chosen.len() < MAX_SAMPLES && inside(frame) && !chosen.contains(&frame) {
            chosen.push(frame);
        }
    };
    for frame in ordered {
        take(frame, &mut chosen);
    }
    let starts: Vec<u64> = target
        .plate_starts
        .iter()
        .copied()
        .filter(|&frame| inside(frame) && !chosen.contains(&frame))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let room = MAX_SAMPLES - chosen.len();
    if starts.len() <= room {
        for frame in starts {
            take(frame, &mut chosen);
        }
    } else if room > 0 {
        for slot in 0..room {
            take(
                starts[slot * (starts.len() - 1) / (room - 1).max(1)],
                &mut chosen,
            );
        }
    }
    chosen.sort_unstable();
    chosen
}

/// Intersection over union of two rectangles given as left, top, right, bottom.
pub fn iou(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> f64 {
    let width = (a.2.min(b.2) - a.0.max(b.0)).max(0.0);
    let height = (a.3.min(b.3) - a.1.max(b.1)).max(0.0);
    let shared = width * height;
    let area = |r: (f64, f64, f64, f64)| (r.2 - r.0).max(0.0) * (r.3 - r.1).max(0.0);
    let union = area(a) + area(b) - shared;
    if union > 0.0 { shared / union } else { 0.0 }
}

#[cfg(test)]
#[path = "tests/samples.rs"]
mod tests;
