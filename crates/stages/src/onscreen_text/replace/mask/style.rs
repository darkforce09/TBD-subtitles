//! Lettering style measured from the original strokes.
//!
//! **Role:** read fill and outline colours, stroke and outline thickness and outline softness
//! from the segmented ink of the keyframe's analysis window.
//! **Position:** called by stroke segmentation once the ink is known.
//! **Signals and state:** pure functions of one window's pixels, clusters and ink.
//! **Invariants:** colours are medians of real source pixels; only core clusters are fill or
//! outline, never the anti-aliased fringe; without a second core cluster the style has no
//! outline, zero outline thickness and a hard edge.

use image::{GrayImage, Luma};
use imageproc::distance_transform::Norm;
use imageproc::morphology::{dilate, erode};
use job_model::onscreen::LetteringStyle;

use super::cluster::{self, Lab};
use super::ink::MIN_INK_SEPARATION;
use super::segment::{Window, in_ring};

/// Width of the band just outside the outline examined for a soft halo, in pixels.
const HALO_PX: u8 = 2;
/// How near the outside of the ink a pixel lies to count toward the outline, in pixels.
const EDGE_PX: u8 = 2;
/// Share of the background-to-outline distance past which a pixel is part of a ramp.
const RAMP_SHARE: f32 = 0.25;
/// How much more often halo pixels ramp than ring pixels for a soft outline.
const SOFT_EXCESS: f64 = 0.40;

/// Measure the style of the kept ink in `window`.
pub(super) fn measure(window: &Window<'_>, line_height: f64) -> LetteringStyle {
    let (w, h) = window.pixels.dimensions();
    let clusters = window.clusters;
    let (fill, outline) = roles(window);
    let fill_mask = cluster_mask(window, fill, w, h);
    let eroded = erode(&fill_mask, Norm::LInf, 1);
    let fill_rgb = median_rgb(window, &eroded).or_else(|| median_rgb(window, &fill_mask));
    let fill_area = count(&fill_mask);
    let perimeter = perimeter(&fill_mask).max(1) as f64;
    let mut style = LetteringStyle {
        fill_rgb: fill_rgb.unwrap_or([255, 255, 255]),
        outline_rgb: None,
        outline_px: 0.0,
        soft_outline: false,
        stroke_px: 2.0 * fill_area as f64 / perimeter,
        line_height_px: line_height,
    };
    if let Some(outline) = outline {
        let outline_mask = cluster_mask(window, outline, w, h);
        style.outline_rgb = median_rgb(window, &outline_mask);
        style.outline_px = count(&outline_mask) as f64 / perimeter;
        style.soft_outline = soft(
            window,
            &outline_mask,
            clusters.centres[usize::from(outline)],
        );
    }
    style
}

/// The fill cluster and, for outlined writing, the outline cluster: of the core cluster with the
/// most kept ink and the next one clearly apart from it in colour, the one lying nearer the
/// outside of the ink is the outline.
pub(super) fn roles(window: &Window<'_>) -> (u8, Option<u8>) {
    let (w, h) = window.pixels.dimensions();
    let clusters = window.clusters;
    let mut kept = vec![0usize; clusters.centres.len()];
    for (_, &label) in window.ink.iter().zip(&clusters.labels).filter(|(k, _)| **k) {
        kept[usize::from(label)] += 1;
    }
    let mut ink_clusters: Vec<u8> = (0..clusters.centres.len() as u8)
        .filter(|&c| window.core[usize::from(c)] && kept[usize::from(c)] > 0)
        .collect();
    ink_clusters.sort_by_key(|&c| std::cmp::Reverse(kept[usize::from(c)]));
    let second = ink_clusters.iter().copied().skip(1).find(|&c| {
        cluster::pair_separation(clusters, usize::from(ink_clusters[0]), usize::from(c))
            >= MIN_INK_SEPARATION
    });
    match (ink_clusters.first().copied(), second) {
        (Some(a), Some(b)) => {
            let outside = dilate(
                &GrayImage::from_fn(w, h, |x, y| {
                    Luma([u8::from(!window.ink[(y * w + x) as usize]) * 255])
                }),
                Norm::LInf,
                EDGE_PX,
            );
            let edge = |c: u8| {
                let near = window
                    .ink
                    .iter()
                    .zip(&clusters.labels)
                    .zip(outside.pixels())
                    .filter(|((k, l), o)| **k && **l == c && o.0[0] > 0)
                    .count();
                near as f64 / kept[usize::from(c)] as f64
            };
            if edge(a) >= edge(b) {
                (b, Some(a))
            } else {
                (a, Some(b))
            }
        }
        (Some(a), None) => (a, None),
        (None, _) => (0, None),
    }
}

/// In-window neighbours of a pixel: 8-connected when `diagonal`, else 4-connected.
fn neighbours(x: u32, y: u32, w: u32, h: u32, diagonal: bool) -> impl Iterator<Item = (u32, u32)> {
    (-1i64..=1)
        .flat_map(|dy| (-1i64..=1).map(move |dx| (dx, dy)))
        .filter(move |&(dx, dy)| (dx, dy) != (0, 0) && (diagonal || dx == 0 || dy == 0))
        .filter_map(move |(dx, dy)| {
            let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
            (nx >= 0 && ny >= 0 && nx < i64::from(w) && ny < i64::from(h))
                .then_some((nx as u32, ny as u32))
        })
}

fn cluster_mask(window: &Window<'_>, cluster: u8, w: u32, h: u32) -> GrayImage {
    GrayImage::from_fn(w, h, |x, y| {
        let i = (y * w + x) as usize;
        Luma([if window.ink[i] && window.clusters.labels[i] == cluster {
            255
        } else {
            0
        }])
    })
}

fn count(mask: &GrayImage) -> usize {
    mask.pixels().filter(|p| p.0[0] > 0).count()
}

/// Mask pixels with a 4-neighbour outside the mask or the window.
fn perimeter(mask: &GrayImage) -> usize {
    let (w, h) = mask.dimensions();
    mask.enumerate_pixels()
        .filter(|(x, y, p)| {
            p.0[0] > 0
                && (*x == 0
                    || *y == 0
                    || *x + 1 == w
                    || *y + 1 == h
                    || neighbours(*x, *y, w, h, false)
                        .any(|(nx, ny)| mask.get_pixel(nx, ny).0[0] == 0))
        })
        .count()
}

/// Per-channel median of the source pixels under `mask`, or `None` for an empty mask.
fn median_rgb(window: &Window<'_>, mask: &GrayImage) -> Option<[u8; 3]> {
    let mut histograms = [[0usize; 256]; 3];
    let mut total = 0usize;
    for (pixel, m) in window.pixels.pixels().zip(mask.pixels()) {
        if m.0[0] > 0 {
            for (histogram, &v) in histograms.iter_mut().zip(&pixel.0) {
                histogram[usize::from(v)] += 1;
            }
            total += 1;
        }
    }
    if total == 0 {
        return None;
    }
    Some(histograms.map(|histogram| {
        let mut seen = 0;
        for (value, &n) in histogram.iter().enumerate() {
            seen += n;
            if 2 * seen > total {
                return value as u8;
            }
        }
        u8::MAX
    }))
}

/// Whether the band just outside the outline fades towards the background more often than the
/// window's own border ring varies.
fn soft(window: &Window<'_>, outline: &GrayImage, outline_centre: Lab) -> bool {
    let (w, h) = outline.dimensions();
    let Some(background) = main_background(window, w, h) else {
        return false;
    };
    let threshold = RAMP_SHARE * cluster::distance(background, outline_centre);
    let ramps = |i: usize| cluster::distance(window.lab[i], background) > threshold;
    let band = dilate(outline, Norm::LInf, HALO_PX);
    let (mut halo, mut halo_ramps, mut ring, mut ring_ramps) = (0usize, 0usize, 0usize, 0usize);
    for (x, y, p) in band.enumerate_pixels() {
        let i = (y * w + x) as usize;
        if p.0[0] > 0 && !window.ink[i] {
            halo += 1;
            halo_ramps += usize::from(ramps(i));
        }
        if in_ring(x, y, w, h) {
            ring += 1;
            ring_ramps += usize::from(ramps(i));
        }
    }
    if halo == 0 || ring == 0 {
        return false;
    }
    halo_ramps as f64 / halo as f64 - ring_ramps as f64 / ring as f64 > SOFT_EXCESS
}

/// The centre of the background cluster holding most of the border ring.
fn main_background(window: &Window<'_>, w: u32, h: u32) -> Option<Lab> {
    let mut ring = vec![0usize; window.clusters.centres.len()];
    for (i, &label) in window.clusters.labels.iter().enumerate() {
        let (x, y) = ((i as u32) % w, (i as u32) / w);
        if in_ring(x, y, w, h) && window.background[usize::from(label)] {
            ring[usize::from(label)] += 1;
        }
    }
    let (index, &most) = ring.iter().enumerate().max_by_key(|(_, n)| **n)?;
    (most > 0).then(|| window.clusters.centres[index])
}
