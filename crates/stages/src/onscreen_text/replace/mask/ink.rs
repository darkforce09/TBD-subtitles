//! Which colours of an analysis window are writing.
//!
//! **Role:** read one colour partition of the keyframe's analysis window and choose its ink:
//! lettering drawn over a background the window's border ring shows, or writing printed on a
//! panel or sign that fills the detector box.
//! **Position:** called by stroke segmentation for every partition; segmentation keeps the best
//! selection that passes its coverage guards.
//! **Signals and state:** pure functions of one partition's labels and the window size.
//! **Invariants:** a selection has at least one core cluster separated from every background
//! cluster (or from the panel) by [`MIN_INK_SEPARATION`] spreads, or a fill and outline wrapping
//! each other; fringe clusters join the ink only beside core ink; an outline colour counts only
//! in the band around its fill; a panel is never ink.

use image::{GrayImage, Luma};
use imageproc::distance_transform::Norm;
use imageproc::morphology::dilate;
use imageproc::region_labelling::{Connectivity, connected_components};

use super::cluster::{self, Clusters, Lab};
use super::segment::in_ring;

/// Least share of the border ring that makes a cluster background.
const BACKGROUND_SHARE: f64 = 0.30;
/// A cluster whose share of the ring is more than this part of its share of the window's inside
/// is background: writing sits inside the box, while texture is everywhere.
const RING_ENRICHMENT: f64 = 0.75;
/// Least distance between a core ink cluster and every background cluster, relative to their
/// spreads; uniform noise splits into clusters closer than this.
pub(super) const MIN_INK_SEPARATION: f32 = 2.0;
/// Least distance from every background cluster, relative to their spreads, of a fill and an
/// outline that wrap each other: the structure of outlined lettering makes up for colours the
/// picture behind a translucent panel comes close to.
const MIN_OUTLINED_SEPARATION: f32 = 1.0;
/// Least share of a fill's pixels within the dilation reach of its outline.
const FILL_WRAPPED: f64 = 0.8;
/// Least share of an outline's pixels within the dilation reach of its fill; the rest may be
/// picture line art of the same colour.
const OUTLINE_HUGS: f64 = 0.5;
/// Least share of the window a core cluster holds.
const MIN_CORE_SHARE: f64 = 0.005;
/// Least share of a weak cluster's pixels lying beside core ink for it to be the writing's fringe
/// or outline rather than picture.
const FRINGE_ADJACENCY: f64 = 0.85;
/// A cluster whose centre lies within this share of a core-to-background distance of the line
/// between them, and at least this share of the way from either end, is a blend of the two:
/// anti-aliasing, never the fill or outline colour.
const BLEND_SHARE: f32 = 0.25;
/// Least share of the quad a panel covers.
const PANEL_SHARE: f64 = 0.5;
/// Least share of the quad a colour the ring does not show, other than the ink, covers for the
/// writing to sit on a panel rather than on the ring's background.
const HIDDEN_SURFACE_SHARE: f64 = 0.3;
/// Least share of the panel's pixels inside the quad held by its largest piece.
const PANEL_SOLIDITY: f64 = 0.8;

/// The ink chosen in one partition.
#[derive(Debug, Clone)]
pub(super) struct Selection {
    /// Per cluster: part of the background, never erased.
    pub background: Vec<bool>,
    /// Per cluster: a colour the writing is drawn in, fill or outline.
    pub core: Vec<bool>,
    /// Per cluster: erased where it forms ink pieces (core and fringe).
    pub ink: Vec<bool>,
    /// Per window pixel: ink-coloured and eligible as writing, before piece filtering.
    pub pixels: Vec<bool>,
    /// The weakest core separation; higher is a cleaner split.
    pub score: f32,
    /// For writing printed on a panel filling the box, the panel's extent in the window as
    /// (left, top, right, bottom), exclusive right and bottom.
    pub panel: Option<(u32, u32, u32, u32)>,
}

/// Lettering over a background shown by the border ring: background clusters crowd the ring,
/// core clusters are rare there and either well separated from all background or a fill and
/// outline wrapping each other, and fringe clusters are weak but lie beside core ink. `inside`
/// flags the window pixels inside the quad; `reach` is the dilation radius. `None` when no core
/// cluster exists, or when a colour the ring does not show covers much of the quad: that is a
/// panel, not lettering.
pub(super) fn lettering(
    clusters: &Clusters,
    w: u32,
    h: u32,
    inside: &[bool],
    reach: u8,
) -> Option<Selection> {
    let k = clusters.centres.len();
    let (ring, within) = shares(clusters, w, h);
    let quad = quad_shares(clusters, inside);
    let mut background: Vec<bool> = (0..k)
        .map(|c| ring[c] > BACKGROUND_SHARE || ring[c] > RING_ENRICHMENT * within[c])
        .collect();
    let shown = background.clone();
    if shown.iter().all(|&b| b) || (0..k).any(|c| !shown[c] && quad[c] >= PANEL_SHARE) {
        return None;
    }
    let total = clusters.labels.len() as f64;
    let sizable = |c: usize| clusters.counts[c] as f64 >= MIN_CORE_SHARE * total;
    let separation: Vec<f32> = (0..k)
        .map(|c| separation_from(clusters, c, &background))
        .collect();
    let mut core: Vec<bool> = (0..k)
        .map(|c| !background[c] && sizable(c) && separation[c] >= MIN_INK_SEPARATION)
        .collect();
    let mut score = (0..k)
        .filter(|&c| core[c])
        .map(|c| separation[c])
        .fold(f32::INFINITY, f32::min);
    let near: Vec<Vec<bool>> = (0..k)
        .map(|c| beside(&clusters.labels, &one(k, c), w, h, reach))
        .collect();
    let edge = beside(&clusters.labels, &background, w, h, 1);
    let outlines = outlined_pairs(clusters, &near, &edge, |c| {
        !background[c] && sizable(c) && separation[c] >= MIN_OUTLINED_SEPARATION
    });
    for &(fill, outline) in &outlines {
        core[fill] = true;
        core[outline] = true;
        score = score.min(separation[fill].min(separation[outline]));
    }
    if !core.iter().any(|&c| c) {
        return None;
    }
    let near_core = beside(&clusters.labels, &core, w, h, reach);
    let mut ink = core.clone();
    for c in 0..k {
        if background[c] || core[c] {
            continue;
        }
        if share_within(clusters, c, &near_core) >= FRINGE_ADJACENCY {
            ink[c] = true;
            core[c] = true;
        } else {
            background[c] = true;
        }
    }
    if (0..k).any(|c| !shown[c] && !ink[c] && quad[c] >= HIDDEN_SURFACE_SHARE) {
        return None;
    }
    let fill_of: Vec<Option<usize>> = (0..k)
        .map(|c| outlines.iter().find(|p| p.1 == c).map(|p| p.0))
        .collect();
    let pixels = clusters
        .labels
        .iter()
        .enumerate()
        .map(|(i, &l)| {
            let c = usize::from(l);
            ink[c] && fill_of[c].is_none_or(|fill| near[fill][i])
        })
        .collect();
    Some(Selection {
        core: without_blends(clusters, &core, &background),
        background,
        ink,
        pixels,
        score,
        panel: None,
    })
}

/// Writing printed on a panel or sign that fills the box: the panel is the cluster covering most
/// of the quad in one solid piece, and the ink is every colour well separated from it. `inside`
/// flags the window pixels inside the quad. `None` when no such panel or colour exists.
pub(super) fn panel(clusters: &Clusters, w: u32, h: u32, inside: &[bool]) -> Option<Selection> {
    let k = clusters.centres.len();
    let mut in_quad = vec![0usize; k];
    for (&label, _) in clusters.labels.iter().zip(inside).filter(|(_, i)| **i) {
        in_quad[usize::from(label)] += 1;
    }
    let quad_total: usize = in_quad.iter().sum();
    let (surface, &most) = in_quad.iter().enumerate().max_by_key(|(_, n)| **n)?;
    if (most as f64) < PANEL_SHARE * quad_total as f64 {
        return None;
    }
    let panel_mask = GrayImage::from_fn(w, h, |x, y| {
        Luma([u8::from(usize::from(clusters.labels[(y * w + x) as usize]) == surface) * 255])
    });
    let pieces = connected_components(&panel_mask, Connectivity::Four, Luma([0u8]));
    let count = pieces.pixels().map(|p| p.0[0]).max().unwrap_or(0) as usize;
    let mut piece_sizes = vec![0usize; count + 1];
    for (p, _) in pieces.pixels().zip(inside).filter(|(_, i)| **i) {
        piece_sizes[p.0[0] as usize] += 1;
    }
    let (solid, &size) = piece_sizes
        .iter()
        .enumerate()
        .skip(1)
        .max_by_key(|(_, n)| **n)?;
    if (size as f64) < PANEL_SOLIDITY * most as f64 {
        return None;
    }
    let extent = pieces
        .enumerate_pixels()
        .filter(|(_, _, p)| p.0[0] as usize == solid)
        .fold((w, h, 0, 0), |(l, t, r, b), (x, y, _)| {
            (l.min(x), t.min(y), r.max(x + 1), b.max(y + 1))
        });
    let background: Vec<bool> = (0..k).map(|c| c == surface).collect();
    let ink: Vec<bool> = (0..k)
        .map(|c| {
            c != surface && cluster::pair_separation(clusters, c, surface) >= MIN_INK_SEPARATION
        })
        .collect();
    if !ink.iter().any(|&c| c) {
        return None;
    }
    let score = (0..k)
        .filter(|&c| ink[c])
        .map(|c| cluster::pair_separation(clusters, c, surface))
        .fold(f32::INFINITY, f32::min);
    Some(Selection {
        core: without_blends(clusters, &ink, &background),
        pixels: clusters
            .labels
            .iter()
            .map(|&l| ink[usize::from(l)])
            .collect(),
        background,
        ink,
        score,
        panel: Some(extent),
    })
}

/// Fill and outline pairs among the `eligible` clusters: well apart in colour, the fill almost
/// wholly within reach of the outline and the outline mostly within reach of the fill. Of two
/// clusters wrapping each other, the one touching the background (`edge`) less is the fill.
fn outlined_pairs(
    clusters: &Clusters,
    near: &[Vec<bool>],
    edge: &[bool],
    eligible: impl Fn(usize) -> bool,
) -> Vec<(usize, usize)> {
    let k = clusters.centres.len();
    let wraps = |fill: usize, outline: usize| {
        share_within(clusters, fill, &near[outline]) >= FILL_WRAPPED
            && share_within(clusters, outline, &near[fill]) >= OUTLINE_HUGS
    };
    let mut pairs = Vec::new();
    for a in 0..k {
        for b in a + 1..k {
            if !(eligible(a)
                && eligible(b)
                && cluster::pair_separation(clusters, a, b) >= MIN_INK_SEPARATION)
            {
                continue;
            }
            let pair = match (wraps(a, b), wraps(b, a)) {
                (true, true)
                    if share_within(clusters, a, edge) > share_within(clusters, b, edge) =>
                {
                    (b, a)
                }
                (true, _) => (a, b),
                (false, true) => (b, a),
                (false, false) => continue,
            };
            pairs.push(pair);
        }
    }
    pairs
}

/// Share of cluster `c`'s pixels flagged in `mask`.
fn share_within(clusters: &Clusters, c: usize, mask: &[bool]) -> f64 {
    let hits = clusters
        .labels
        .iter()
        .zip(mask)
        .filter(|(l, m)| usize::from(**l) == c && **m)
        .count();
    hits as f64 / clusters.counts[c].max(1) as f64
}

/// Each cluster's share of the border ring and of the window inside it.
fn shares(clusters: &Clusters, w: u32, h: u32) -> (Vec<f64>, Vec<f64>) {
    let k = clusters.centres.len();
    let (mut ring, mut inside) = (vec![0usize; k], vec![0usize; k]);
    for (i, &label) in clusters.labels.iter().enumerate() {
        let (x, y) = ((i as u32) % w, (i as u32) / w);
        if in_ring(x, y, w, h) {
            ring[usize::from(label)] += 1;
        } else {
            inside[usize::from(label)] += 1;
        }
    }
    (to_shares(ring), to_shares(inside))
}

/// Each cluster's share of the window pixels inside the quad.
fn quad_shares(clusters: &Clusters, inside: &[bool]) -> Vec<f64> {
    let mut counts = vec![0usize; clusters.centres.len()];
    for (&label, _) in clusters.labels.iter().zip(inside).filter(|(_, i)| **i) {
        counts[usize::from(label)] += 1;
    }
    to_shares(counts)
}

fn to_shares(counts: Vec<usize>) -> Vec<f64> {
    let total = counts.iter().sum::<usize>().max(1) as f64;
    counts.into_iter().map(|n| n as f64 / total).collect()
}

/// Flags for `k` clusters with only `c` set.
fn one(k: usize, c: usize) -> Vec<bool> {
    (0..k).map(|i| i == c).collect()
}

/// The closest background cluster to `c`, in spreads; infinite without background.
fn separation_from(clusters: &Clusters, c: usize, background: &[bool]) -> f32 {
    (0..clusters.centres.len())
        .filter(|&j| background[j] && j != c)
        .map(|j| cluster::pair_separation(clusters, c, j))
        .fold(f32::INFINITY, f32::min)
}

/// Window pixels within `reach` pixels (chessboard) of a pixel of a `flagged` cluster.
fn beside(labels: &[u8], flagged: &[bool], w: u32, h: u32, reach: u8) -> Vec<bool> {
    let mask = GrayImage::from_fn(w, h, |x, y| {
        Luma([u8::from(flagged[usize::from(labels[(y * w + x) as usize])]) * 255])
    });
    dilate(&mask, Norm::LInf, reach)
        .pixels()
        .map(|p| p.0[0] > 0)
        .collect()
}

/// The `core` clusters that are colours of their own: taken largest first, a cluster lying
/// between an already kept core colour and a background colour is their blend (anti-aliasing)
/// and stays ink without being fill or outline.
fn without_blends(clusters: &Clusters, core: &[bool], background: &[bool]) -> Vec<bool> {
    let k = clusters.centres.len();
    let mut order: Vec<usize> = (0..k).filter(|&c| core[c]).collect();
    order.sort_by_key(|&c| std::cmp::Reverse(clusters.counts[c]));
    let mut kept = vec![false; k];
    for c in order {
        let point = clusters.centres[c];
        let blend = (0..k).filter(|&a| kept[a]).any(|a| {
            (0..k)
                .filter(|&b| background[b])
                .any(|b| near_segment(point, clusters.centres[a], clusters.centres[b]))
        });
        kept[c] = !blend;
    }
    kept
}

/// Whether `point` lies strictly between `a` and `b`: past [`BLEND_SHARE`] of the way from
/// either end, and within that share of the segment's length from it.
fn near_segment(point: Lab, a: Lab, b: Lab) -> bool {
    let along: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
    let length = cluster::distance(a, b);
    if length <= f32::EPSILON {
        return false;
    }
    let offset: [f32; 3] = std::array::from_fn(|i| point[i] - a[i]);
    let t = offset.iter().zip(&along).map(|(o, d)| o * d).sum::<f32>() / (length * length);
    if !(BLEND_SHARE..=1.0 - BLEND_SHARE).contains(&t) {
        return false;
    }
    let closest: Lab = std::array::from_fn(|i| a[i] + t * along[i]);
    cluster::distance(point, closest) < BLEND_SHARE * length
}

#[cfg(test)]
#[path = "tests/ink.rs"]
mod tests;
