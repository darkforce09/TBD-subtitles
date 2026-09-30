//! One replacement per sign.
//!
//! **Role:** among the occurrences about to be lettered, find those another one already covers
//! (the same writing seen twice, by the detector and by Claude, or two readings of one sign) so
//! only one patch is ever drawn over a piece of writing.
//! **Position:** under `compose`, after each pending occurrence is prepared and before
//! containers are grouped and lettered.
//! **Signals and state:** pure function of frame spans and lettering areas.
//! **Invariants:** deterministic; the kept occurrence of a covering pair is the detector's
//! before Claude's, then the longer on screen, then the larger, then the earlier in the document;
//! no two kept occurrences on screen together cover each other.

use job_model::onscreen::Quad;

/// Why an occurrence another replacement covers stays in the subtitle file.
pub(crate) const COVERED: &str = "Another replacement covers this writing";
/// Intersection over union of two lettering areas at which they are one sign.
const SAME_SIGN_IOU: f64 = 0.3;
/// Share of the smaller lettering area the larger covers at which they are one sign.
const SAME_SIGN_COVER: f64 = 0.5;

/// One occurrence about to be lettered, as the overlap check sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Claim {
    pub first_frame: u64,
    pub last_frame: u64,
    /// The lettering area at the keyframe.
    pub quad: Quad,
    /// Found by Claude on a keyframe rather than by the local detector.
    pub found_by_claude: bool,
}

/// Per claim, whether a kept claim covers it: both on screen together and their lettering areas
/// overlapping by [`SAME_SIGN_IOU`] or covering [`SAME_SIGN_COVER`] of the smaller.
pub(crate) fn covered(claims: &[Claim]) -> Vec<bool> {
    let mut order: Vec<usize> = (0..claims.len()).collect();
    order.sort_by(|&a, &b| {
        let (ca, cb) = (&claims[a], &claims[b]);
        ca.found_by_claude
            .cmp(&cb.found_by_claude)
            .then((cb.last_frame - cb.first_frame).cmp(&(ca.last_frame - ca.first_frame)))
            .then(area(cb.quad).total_cmp(&area(ca.quad)))
            .then(a.cmp(&b))
    });
    let mut kept: Vec<usize> = Vec::new();
    let mut covered = vec![false; claims.len()];
    for index in order {
        if kept.iter().any(|&k| same_sign(&claims[k], &claims[index])) {
            covered[index] = true;
        } else {
            kept.push(index);
        }
    }
    covered
}

fn same_sign(a: &Claim, b: &Claim) -> bool {
    if a.first_frame > b.last_frame || b.first_frame > a.last_frame {
        return false;
    }
    let ((al, at, ar, ab), (bl, bt, br, bb)) = (a.quad.bounds(), b.quad.bounds());
    let common = (ar.min(br) - al.max(bl)).max(0.0) * (ab.min(bb) - at.max(bt)).max(0.0);
    let (area_a, area_b) = (area(a.quad), area(b.quad));
    let union = area_a + area_b - common;
    common > 0.0
        && (common >= SAME_SIGN_IOU * union || common >= SAME_SIGN_COVER * area_a.min(area_b))
}

/// The area of a quad's bounds.
fn area(quad: Quad) -> f64 {
    let (l, t, r, b) = quad.bounds();
    (r - l).max(0.0) * (b - t).max(0.0)
}

#[cfg(test)]
#[path = "tests/overlap.rs"]
mod tests;
