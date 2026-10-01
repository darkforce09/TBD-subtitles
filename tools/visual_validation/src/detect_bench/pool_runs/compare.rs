//! How far the boxes one detector run found on a set of frames differ from a reference run's.
//!
//! **Role:** count, frame by frame, whether two runs found exactly the same regions (the same
//! corners and scores in the same order), how many boxes each found, and how many boxes of the
//! run pair up one to one with a reference box whose bounding rectangle overlaps it at an
//! intersection over union of at least one half; describe the result in one table cell.
//! **Position:** used by the pool sections of `detect-bench` for the search modes and for
//! TensorRT against the CUDA path.
//! **Signals and state:** none; pure functions over region lists.
//! **Invariants:** a run compared with itself is identical with every box matched; frames are
//! compared up to the shorter run's length and a length difference is never identical.

use job_model::onscreen::Quad;

/// The regions one run found, one list per frame in frame order.
pub type Regions = Vec<Vec<(Quad, f64)>>;

/// The intersection over union two boxes' bounding rectangles need to count as the same box.
pub const MATCH_IOU: f64 = 0.5;

/// How one run's boxes compare with a reference run's on the same frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comparison {
    pub frames: usize,
    pub identical_frames: usize,
    pub boxes: usize,
    pub reference_boxes: usize,
    /// Boxes paired one to one with a reference box at `MATCH_IOU` or more.
    pub matched: usize,
    /// Whether both runs covered the same number of frames.
    pub same_length: bool,
}

impl Comparison {
    /// Whether every frame's regions equal the reference's exactly.
    pub fn identical(&self) -> bool {
        self.same_length && self.identical_frames == self.frames
    }
}

/// `found` against `reference`, frame by frame.
pub fn compare(found: &[Vec<(Quad, f64)>], reference: &[Vec<(Quad, f64)>]) -> Comparison {
    let frames = found.len().min(reference.len());
    let mut comparison = Comparison {
        frames,
        identical_frames: 0,
        boxes: 0,
        reference_boxes: 0,
        matched: 0,
        same_length: found.len() == reference.len(),
    };
    for (ours, theirs) in found.iter().zip(reference) {
        comparison.identical_frames += usize::from(ours == theirs);
        comparison.boxes += ours.len();
        comparison.reference_boxes += theirs.len();
        comparison.matched += matched(ours, theirs);
    }
    comparison
}

/// How many of `ours` pair one to one with a box of `theirs`, greedily by best overlap.
fn matched(ours: &[(Quad, f64)], theirs: &[(Quad, f64)]) -> usize {
    let mut taken = vec![false; theirs.len()];
    let mut count = 0;
    for (quad, _) in ours {
        let best = theirs
            .iter()
            .enumerate()
            .filter(|(index, _)| !taken[*index])
            .map(|(index, (other, _))| (index, iou(*quad, *other)))
            .filter(|(_, overlap)| *overlap >= MATCH_IOU)
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((index, _)) = best {
            taken[index] = true;
            count += 1;
        }
    }
    count
}

/// The intersection over union of two boxes' bounding rectangles; zero for empty ones.
pub fn iou(a: Quad, b: Quad) -> f64 {
    let (al, at, ar, ab) = a.bounds();
    let (bl, bt, br, bb) = b.bounds();
    let width = (ar.min(br) - al.max(bl)).max(0.0);
    let height = (ab.min(bb) - at.max(bt)).max(0.0);
    let inter = width * height;
    let union = (ar - al) * (ab - at) + (br - bl) * (bb - bt) - inter;
    if union > 0.0 { inter / union } else { 0.0 }
}

/// One cell: `identical` with the box count, or what differs.
pub fn describe(comparison: &Comparison) -> String {
    if comparison.identical() {
        return format!(
            "identical: {} boxes on {} frames",
            comparison.boxes, comparison.frames
        );
    }
    let mut text = format!(
        "differs: {} of {} frames identical; {} boxes against {}; {} matched at IoU ≥ {MATCH_IOU}",
        comparison.identical_frames,
        comparison.frames,
        comparison.boxes,
        comparison.reference_boxes,
        comparison.matched
    );
    if !comparison.same_length {
        text.push_str("; the runs covered different frame counts");
    }
    text
}

#[cfg(test)]
#[path = "tests/compare.rs"]
mod tests;
