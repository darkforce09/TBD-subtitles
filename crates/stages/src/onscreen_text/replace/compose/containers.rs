//! Keyframe geometry and the grouping of neighbouring writing into shared containers.
//!
//! **Role:** find each occurrence's keyframe quad, its place on every plate, and which
//! occurrences share one card and therefore one size ratio.
//! **Position:** geometry helpers under `compose`, before layout.
//! **Signals and state:** occurrence frames and plate placements in; quads and groups out.
//! **Invariants:** grouping is deterministic and follows document order; a group needs
//! overlapping frame spans and quads within one line height of each other.

use job_model::onscreen::{Plate, Point, Quad, ReplacedText, TextFrame, TextOccurrence};

use super::layout::Area;
use crate::onscreen_text::geometry;

/// One pending occurrence as the grouping sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Member {
    pub first_frame: u64,
    pub last_frame: u64,
    pub quad: Quad,
    pub line_height: f64,
}

/// The tracked frame shown at the keyframe: the one whose interval contains its time, else the
/// nearest; the middle of the occurrence stands in when it has no keyframe.
pub(crate) fn keyframe_frame(occurrence: &TextOccurrence) -> Option<&TextFrame> {
    let time = occurrence
        .keyframe
        .as_ref()
        .map(|keyframe| keyframe.time_s)
        .unwrap_or((occurrence.start_s + occurrence.end_s) / 2.0);
    let distance = |frame: &TextFrame| {
        if frame.time_s <= time && time < frame.end_s {
            0.0
        } else {
            (frame.time_s - time).abs().min((frame.end_s - time).abs())
        }
    };
    occurrence
        .frames
        .iter()
        .filter(|frame| frame.quad.valid())
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
}

/// The frame index of the keyframe inside the text's frame span.
pub(crate) fn keyframe_index(text: &ReplacedText, occurrence: &TextOccurrence) -> u64 {
    let span_s = occurrence.end_s - occurrence.start_s;
    let frames = text.last_frame - text.first_frame + 1;
    let time = occurrence
        .keyframe
        .as_ref()
        .map(|keyframe| keyframe.time_s)
        .unwrap_or((occurrence.start_s + occurrence.end_s) / 2.0);
    if !(span_s.is_finite() && span_s > 0.0) {
        return text.first_frame;
    }
    let offset = ((time - occurrence.start_s) / span_s * frames as f64).floor();
    let offset = if offset.is_finite() {
        offset.max(0.0)
    } else {
        0.0
    } as u64;
    text.first_frame + offset.min(frames - 1)
}

/// The plate covering `frame`, else the plate nearest to it.
pub(crate) fn plate_at(plates: &[Plate], frame: u64) -> Option<usize> {
    let gap = |plate: &Plate| {
        if frame < plate.first_frame {
            plate.first_frame - frame
        } else {
            frame.saturating_sub(plate.last_frame)
        }
    };
    (0..plates.len()).min_by_key(|&index| gap(&plates[index]))
}

/// The keyframe quad placed on a plate: scaled about its centre, shifted, in plate pixels.
pub(crate) fn plate_quad(quad: Quad, plate: &Plate) -> Quad {
    let centre = quad.center();
    Quad(quad.0.map(|point| Point {
        x: centre.x + (point.x - centre.x) * plate.scale + plate.shift[0] - f64::from(plate.rect.x),
        y: centre.y + (point.y - centre.y) * plate.scale + plate.shift[1] - f64::from(plate.rect.y),
    }))
}

/// The rectified size of a quad: mean of the top and bottom edges by mean of the sides.
pub(crate) fn rectified(quad: Quad) -> Area {
    let [tl, tr, br, bl] = quad.0;
    Area {
        width: (geometry::distance(tl, tr) + geometry::distance(bl, br)) / 2.0,
        height: (geometry::distance(tl, bl) + geometry::distance(tr, br)) / 2.0,
    }
}

/// Groups of member positions drawn as one container, each in ascending order and the groups
/// ordered by their first member.
pub(crate) fn group(members: &[Member]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..members.len()).collect();
    for a in 0..members.len() {
        for b in a + 1..members.len() {
            if neighbours(&members[a], &members[b]) {
                let (root_a, root_b) = (find(&mut parent, a), find(&mut parent, b));
                let (low, high) = (root_a.min(root_b), root_a.max(root_b));
                parent[high] = low;
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut slot_of_root = vec![usize::MAX; members.len()];
    for index in 0..members.len() {
        let root = find(&mut parent, index);
        if slot_of_root[root] == usize::MAX {
            slot_of_root[root] = groups.len();
            groups.push(Vec::new());
        }
        groups[slot_of_root[root]].push(index);
    }
    groups
}

fn neighbours(a: &Member, b: &Member) -> bool {
    let spans = a.first_frame <= b.last_frame && b.first_frame <= a.last_frame;
    let (al, at, ar, ab) = expanded(a);
    let (bl, bt, br, bb) = expanded(b);
    spans && al <= br && bl <= ar && at <= bb && bt <= ab
}

fn expanded(member: &Member) -> (f64, f64, f64, f64) {
    let (l, t, r, b) = member.quad.bounds();
    let pad = member.line_height.max(0.0);
    (l - pad, t - pad, r + pad, b + pad)
}

fn find(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

#[cfg(test)]
#[path = "tests/containers.rs"]
mod tests;
