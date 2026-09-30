//! Colour clustering of a writing region in CIE Lab.
//!
//! **Role:** partition a region's pixels into two to five colour clusters, one partition per
//! cluster count, so segmentation can pick the one that isolates the writing best.
//! **Position:** used by stroke segmentation on the keyframe's analysis window.
//! **Signals and state:** pure functions over Lab pixel lists.
//! **Invariants:** deterministic for the same pixels; at most 20 Lloyd iterations per start, fitted
//! on at most 65,536 evenly strided pixels and then applied to all; returned clusters are
//! non-empty and labels index them.

/// A colour in CIE L*a*b* (D65).
pub(super) type Lab = [f32; 3];

/// Most Lloyd iterations per start.
const MAX_ITERATIONS: usize = 20;
/// Cluster counts tried, fewest first.
pub(super) const CLUSTER_COUNTS: std::ops::RangeInclusive<usize> = 2..=5;
/// Most pixels the centres are fitted on; larger regions are sampled with an even stride.
const MAX_FIT_PIXELS: usize = 65_536;
/// Spread added to every pair in the separation score, in Lab units, so identical colours still
/// compare by distance.
const NOISE_FLOOR: f32 = 1.0;

/// Pixels grouped by colour.
#[derive(Debug, Clone)]
pub(super) struct Clusters {
    pub centres: Vec<Lab>,
    /// The cluster of every pixel, in input order.
    pub labels: Vec<u8>,
    pub counts: Vec<usize>,
    /// Root-mean-square distance of each cluster's pixels to its centre.
    pub spread: Vec<f32>,
    /// Sum of squared distances of all pixels to their centres.
    pub cost: f64,
}

/// Convert an sRGB colour to CIE L*a*b* under D65.
pub(super) fn lab(rgb: [u8; 3]) -> Lab {
    let [r, g, b] = rgb.map(linear);
    let x = (0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b) / 0.950_47;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175 * b;
    let z = (0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b) / 1.088_83;
    let (fx, fy, fz) = (pivot(x), pivot(y), pivot(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn linear(channel: u8) -> f32 {
    let v = f32::from(channel) / 255.0;
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn pivot(t: f32) -> f32 {
    if t > 0.008_856 {
        t.cbrt()
    } else {
        7.787 * t + 16.0 / 116.0
    }
}

/// Euclidean distance in Lab (CIE76 ΔE).
pub(super) fn distance(a: Lab, b: Lab) -> f32 {
    squared(a, b).sqrt()
}

fn squared(a: Lab, b: Lab) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// One partition per cluster count in [`CLUSTER_COUNTS`], in that order; empty for no pixels.
/// A count larger than the number of distinct colours yields fewer clusters.
pub(super) fn partitions(pixels: &[Lab]) -> Vec<Clusters> {
    if pixels.is_empty() {
        return Vec::new();
    }
    let stride = pixels.len().div_ceil(MAX_FIT_PIXELS).max(1);
    let sample: Vec<Lab> = pixels.iter().step_by(stride).copied().collect();
    CLUSTER_COUNTS
        .map(|k| best_of(pixels, &sample, k))
        .collect()
}

/// How far apart clusters `i` and `j` sit relative to their spreads.
pub(super) fn pair_separation(clusters: &Clusters, i: usize, j: usize) -> f32 {
    distance(clusters.centres[i], clusters.centres[j])
        / (clusters.spread[i] + clusters.spread[j] + NOISE_FLOOR)
}

/// Lloyd's algorithm on `sample` from two deterministic starts, applied to every pixel, keeping
/// the lower cost.
fn best_of(pixels: &[Lab], sample: &[Lab], k: usize) -> Clusters {
    let from_quantiles = lloyd(sample, luminance_quantiles(sample, k));
    let from_extremes = lloyd(sample, farthest_points(sample, k));
    let from_quantiles = assign(pixels, &from_quantiles.centres);
    let from_extremes = assign(pixels, &from_extremes.centres);
    if from_extremes.cost < from_quantiles.cost {
        from_extremes
    } else {
        from_quantiles
    }
}

/// Every pixel labelled by its nearest centre, with centres and spreads measured from the labels.
fn assign(pixels: &[Lab], centres: &[Lab]) -> Clusters {
    let labels = pixels.iter().map(|p| nearest_centre(*p, centres)).collect();
    compact(pixels, labels, centres.len())
}

/// Starts at the pixels of evenly spaced lightness ranks.
fn luminance_quantiles(pixels: &[Lab], k: usize) -> Vec<Lab> {
    let mut order: Vec<usize> = (0..pixels.len()).collect();
    order.sort_by(|&a, &b| pixels[a][0].total_cmp(&pixels[b][0]));
    (0..k)
        .map(|i| {
            let rank = ((i as f64 + 0.5) / k as f64 * pixels.len() as f64) as usize;
            pixels[order[rank.min(pixels.len() - 1)]]
        })
        .collect()
}

/// Starts at the median-lightness pixel, then repeatedly at the pixel farthest from every start.
fn farthest_points(pixels: &[Lab], k: usize) -> Vec<Lab> {
    let mut centres = vec![luminance_quantiles(pixels, 1)[0]];
    let mut nearest: Vec<f32> = pixels.iter().map(|p| squared(*p, centres[0])).collect();
    while centres.len() < k {
        let (index, _) =
            nearest.iter().enumerate().fold(
                (0, -1.0f32),
                |best, (i, &d)| if d > best.1 { (i, d) } else { best },
            );
        let centre = pixels[index];
        centres.push(centre);
        for (d, p) in nearest.iter_mut().zip(pixels) {
            *d = d.min(squared(*p, centre));
        }
    }
    centres
}

pub(super) fn nearest_centre(pixel: Lab, centres: &[Lab]) -> u8 {
    let mut best = 0;
    let mut best_d = f32::INFINITY;
    for (i, c) in centres.iter().enumerate() {
        let d = squared(pixel, *c);
        if d < best_d {
            best = i;
            best_d = d;
        }
    }
    best as u8
}

fn lloyd(pixels: &[Lab], mut centres: Vec<Lab>) -> Clusters {
    let k = centres.len();
    let mut labels = vec![u8::MAX; pixels.len()];
    for _ in 0..MAX_ITERATIONS {
        let mut changed = false;
        for (label, pixel) in labels.iter_mut().zip(pixels) {
            let nearest = nearest_centre(*pixel, &centres);
            changed |= *label != nearest;
            *label = nearest;
        }
        if !changed {
            break;
        }
        let (sums, counts) = sums(pixels, &labels, k);
        for (c, (sum, count)) in centres.iter_mut().zip(sums.iter().zip(&counts)) {
            if *count > 0 {
                *c = sum.map(|v| (v / *count as f64) as f32);
            }
        }
    }
    compact(pixels, labels, k)
}

fn sums(pixels: &[Lab], labels: &[u8], k: usize) -> (Vec<[f64; 3]>, Vec<usize>) {
    let mut sums = vec![[0.0f64; 3]; k];
    let mut counts = vec![0usize; k];
    for (pixel, &label) in pixels.iter().zip(labels) {
        let sum = &mut sums[usize::from(label)];
        for (s, v) in sum.iter_mut().zip(pixel) {
            *s += f64::from(*v);
        }
        counts[usize::from(label)] += 1;
    }
    (sums, counts)
}

/// Drop empty clusters, relabel, and measure centres, spreads and cost from the final labels.
fn compact(pixels: &[Lab], mut labels: Vec<u8>, k: usize) -> Clusters {
    let (sums, counts) = sums(pixels, &labels, k);
    let mut remap = vec![0u8; k];
    let mut centres = Vec::new();
    let mut kept_counts = Vec::new();
    for i in 0..k {
        if counts[i] > 0 {
            remap[i] = centres.len() as u8;
            centres.push(sums[i].map(|v| (v / counts[i] as f64) as f32));
            kept_counts.push(counts[i]);
        }
    }
    let mut squares = vec![0.0f64; centres.len()];
    for (label, pixel) in labels.iter_mut().zip(pixels) {
        *label = remap[usize::from(*label)];
        squares[usize::from(*label)] += f64::from(squared(*pixel, centres[usize::from(*label)]));
    }
    let spread = squares
        .iter()
        .zip(&kept_counts)
        .map(|(s, &n)| (s / n as f64).sqrt() as f32)
        .collect();
    Clusters {
        centres,
        labels,
        counts: kept_counts,
        spread,
        cost: squares.iter().sum(),
    }
}

#[cfg(test)]
#[path = "tests/cluster.rs"]
mod tests;
