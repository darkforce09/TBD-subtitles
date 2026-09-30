//! Which colours of an analysis window are writing.
//!
//! **Role:** read one colour partition of the keyframe's analysis window and choose its ink:
//! lettering drawn over a background the window's border ring shows, or outlined lettering whose
//! colours also run along the ring; `panel.rs` reads writing printed on a panel.
//! **Position:** called by stroke segmentation for every partition; segmentation keeps the best
//! selection that passes its coverage guards.
//! **Signals and state:** pure functions of one partition's labels and the window size.
//! **Invariants:** a selection has at least one core cluster separated from every background
//! cluster by [`MIN_INK_SEPARATION`] spreads, or a fill and outline wrapping
//! each other; fringe clusters join the ink only beside core ink; an outline colour counts only
//! in the band around its fill; a colour the ring does not show covering much of the quad is a
//! panel and ends the reading.

use image::{GrayImage, Luma};
use imageproc::distance_transform::Norm;
use imageproc::morphology::dilate;

use super::cluster::{self, Clusters, Lab};
use super::pieces;
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
/// Largest spread, in Lab units, of a fill or outline colour that also runs along the ring:
/// lettering is drawn in flat colours, while a wide cluster merges picture colours with it.
const MAX_RING_OUTLINE_SPREAD: f32 = 15.0;
/// Or at most this share of the distance between the fill and outline colours: the anti-aliased
/// edge pixels between strongly contrasting fill and outline widen both clusters.
const RING_OUTLINE_SPREAD_SHARE: f32 = 0.2;
/// Least distance from every other ring colour, relative to their spreads, of a fill and outline
/// wider than [`MAX_RING_OUTLINE_SPREAD`]: a wider colour needs a cleaner split.
const MIN_WIDE_OUTLINED_SEPARATION: f32 = 1.25;
/// Least share of such a fill's pixels with at least three of their four neighbours in the fill:
/// strokes are solid, while a colour scattered through noise or texture is not.
const MIN_SOLID_FILL: f64 = 0.5;
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
pub(super) const PANEL_SHARE: f64 = 0.5;
/// Least share of the quad a colour the ring does not show, other than the ink, covers for the
/// writing to sit on a panel rather than on the ring's background.
const HIDDEN_SURFACE_SHARE: f64 = 0.3;

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
    /// Per window pixel: fill of an outlined pair in a piece its outline does not ring, which is
    /// picture in the fill's colour and never ink.
    pub unwrapped: Vec<bool>,
    /// The weakest core separation; higher is a cleaner split.
    pub score: f32,
    /// For writing printed on a panel filling the box, the panel's extent in the window as
    /// (left, top, right, bottom), exclusive right and bottom.
    pub panel: Option<(u32, u32, u32, u32)>,
    /// Whether the ink holds a fill and outline wrapping each other: bold outlined lettering
    /// fills a tight box more densely than plain writing.
    pub outlined: bool,
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
    read_lettering(clusters, (w, h), inside, reach, false)
}

/// Outlined lettering whose fill and outline colours also run along the box's edges, as when
/// the box grazes a card's outlined frame or a picture of the fill's colour: a fill and outline
/// wrapping each other are the only ink, whatever share of the ring they hold, when both stay
/// [`MIN_OUTLINED_SEPARATION`] spreads from every other ring colour. Segmentation reads this
/// only when neither the lettering nor the panel reading passes.
pub(super) fn outlined_over_ring(
    clusters: &Clusters,
    w: u32,
    h: u32,
    inside: &[bool],
    reach: u8,
) -> Option<Selection> {
    read_lettering(clusters, (w, h), inside, reach, true)
}

/// [`lettering`], or with `ring_outlines` [`outlined_over_ring`].
fn read_lettering(
    clusters: &Clusters,
    (w, h): (u32, u32),
    inside: &[bool],
    reach: u8,
    ring_outlines: bool,
) -> Option<Selection> {
    let k = clusters.centres.len();
    let (ring, within) = shares(clusters, w, h);
    let quad = quad_shares(clusters, inside);
    let mut background: Vec<bool> = (0..k)
        .map(|c| ring[c] > BACKGROUND_SHARE || ring[c] > RING_ENRICHMENT * within[c])
        .collect();
    let mut shown = background.clone();
    if (!ring_outlines && shown.iter().all(|&b| b))
        || (0..k).any(|c| !shown[c] && quad[c] >= PANEL_SHARE)
    {
        return None;
    }
    let total = clusters.labels.len() as f64;
    let sizable = |c: usize| clusters.counts[c] as f64 >= MIN_CORE_SHARE * total;
    let separation: Vec<f32> = (0..k)
        .map(|c| separation_from(clusters, c, &background))
        .collect();
    let mut core: Vec<bool> = (0..k)
        .map(|c| {
            !ring_outlines && !background[c] && sizable(c) && separation[c] >= MIN_INK_SEPARATION
        })
        .collect();
    let mut score = (0..k)
        .filter(|&c| core[c])
        .map(|c| separation[c])
        .fold(f32::INFINITY, f32::min);
    let near: Vec<Vec<bool>> = (0..k)
        .map(|c| beside(&clusters.labels, &one(k, c), w, h, reach))
        .collect();
    let outlines = if ring_outlines {
        let pairs = outlined_pairs(clusters, &near, sizable, |a, b| {
            let others: Vec<bool> = (0..k).map(|c| background[c] && c != a && c != b).collect();
            beside(&clusters.labels, &others, w, h, 1)
        });
        let mut kept = Vec::new();
        for (fill, outline) in pairs {
            let behind = background_beside(clusters, (w, h), [fill, outline]);
            let rest: Vec<bool> = (0..k)
                .map(|c| (background[c] || behind[c]) && c != fill && c != outline)
                .collect();
            let distance = separation_from(clusters, fill, &rest)
                .min(separation_from(clusters, outline, &rest));
            let spread = clusters.spread[fill].max(clusters.spread[outline]);
            let contrast = cluster::distance(clusters.centres[fill], clusters.centres[outline]);
            let least = if spread <= MAX_RING_OUTLINE_SPREAD {
                MIN_OUTLINED_SEPARATION
            } else {
                MIN_WIDE_OUTLINED_SEPARATION
            };
            if distance >= least
                && spread <= MAX_RING_OUTLINE_SPREAD.max(RING_OUTLINE_SPREAD_SHARE * contrast)
                && solid_share(&clusters.labels, fill, w, h) >= MIN_SOLID_FILL
            {
                let paired = |c: usize| kept.iter().any(|&(f, o)| f == c || o == c);
                for c in (0..k).filter(|&c| rest[c] && !paired(c)) {
                    background[c] = true;
                    shown[c] = true;
                }
                background[fill] = false;
                background[outline] = false;
                score = score.min(distance);
                kept.push((fill, outline));
            }
        }
        kept
    } else {
        let edge = beside(&clusters.labels, &background, w, h, 1);
        let eligible =
            |c: usize| !background[c] && sizable(c) && separation[c] >= MIN_OUTLINED_SEPARATION;
        let pairs = outlined_pairs(clusters, &near, eligible, |_, _| edge.clone());
        for &(fill, outline) in &pairs {
            score = score.min(separation[fill].min(separation[outline]));
        }
        pairs
    };
    for &(fill, outline) in &outlines {
        core[fill] = true;
        core[outline] = true;
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
    let mut unwrapped = vec![false; clusters.labels.len()];
    for &(fill, outline) in &outlines {
        let fill_pixels: Vec<bool> = clusters
            .labels
            .iter()
            .map(|&l| usize::from(l) == fill)
            .collect();
        for (flag, apart) in
            unwrapped
                .iter_mut()
                .zip(pieces::unwrapped(&fill_pixels, &near[outline], w, h))
        {
            *flag |= apart;
        }
    }
    Some(Selection {
        core: without_blends(clusters, &core, &background),
        background,
        ink,
        pixels,
        unwrapped,
        score,
        panel: None,
        outlined: !outlines.is_empty(),
    })
}

/// Fill and outline pairs among the `eligible` clusters: well apart in colour, the fill almost
/// wholly within reach of the outline and the outline mostly within reach of the fill. Of two
/// clusters wrapping each other, the one touching the background (`edge_of` the pair) less is
/// the fill.
fn outlined_pairs(
    clusters: &Clusters,
    near: &[Vec<bool>],
    eligible: impl Fn(usize) -> bool,
    edge_of: impl Fn(usize, usize) -> Vec<bool>,
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
                (true, true) => {
                    let edge = edge_of(a, b);
                    if share_within(clusters, a, &edge) > share_within(clusters, b, &edge) {
                        (b, a)
                    } else {
                        (a, b)
                    }
                }
                (true, false) => (a, b),
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

/// Share of cluster `c`'s pixels with at least three of their four in-window neighbours in `c`.
fn solid_share(labels: &[u8], c: usize, w: u32, h: u32) -> f64 {
    let at = |x: u32, y: u32| usize::from(labels[(y * w + x) as usize]) == c;
    let (mut pixels, mut solid) = (0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            if !at(x, y) {
                continue;
            }
            pixels += 1;
            let same = [
                x > 0 && at(x - 1, y),
                x + 1 < w && at(x + 1, y),
                y > 0 && at(x, y - 1),
                y + 1 < h && at(x, y + 1),
            ];
            solid += usize::from(same.iter().filter(|&&s| s).count() >= 3);
        }
    }
    solid as f64 / pixels.max(1) as f64
}

/// The clusters that are background once the pixels of the `pair` (a fill and outline running
/// along the ring) are set aside: each other cluster's share of the rest of the ring against its
/// share of the rest of the window inside it, by the same rule as over the whole ring. Lettering
/// crossing the ring crowds out the background there, so a picture colour behind a card would
/// otherwise read as a hidden panel. The pair itself is never flagged.
fn background_beside(clusters: &Clusters, (w, h): (u32, u32), pair: [usize; 2]) -> Vec<bool> {
    let k = clusters.centres.len();
    let (mut ring, mut inside) = (vec![0usize; k], vec![0usize; k]);
    for (i, &label) in clusters.labels.iter().enumerate() {
        let c = usize::from(label);
        if pair.contains(&c) {
            continue;
        }
        let (x, y) = ((i as u32) % w, (i as u32) / w);
        if in_ring(x, y, w, h) {
            ring[c] += 1;
        } else {
            inside[c] += 1;
        }
    }
    let (ring, within) = (to_shares(ring), to_shares(inside));
    (0..k)
        .map(|c| {
            !pair.contains(&c)
                && (ring[c] > BACKGROUND_SHARE || ring[c] > RING_ENRICHMENT * within[c])
        })
        .collect()
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
pub(super) fn without_blends(clusters: &Clusters, core: &[bool], background: &[bool]) -> Vec<bool> {
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
