//! Writing printed on a panel or sign that fills the box.
//!
//! **Role:** read one colour partition of the keyframe's analysis window as a panel with writing
//! on it: the panel is the colour covering most of the quad in one solid piece, and the ink is
//! every colour well separated from it.
//! **Position:** the second reading stroke segmentation tries, after lettering over the ring's
//! background fails for every partition.
//! **Signals and state:** pure functions of one partition's labels and the window size.
//! **Invariants:** the panel is never ink; every ink colour lies at least
//! [`MIN_INK_SEPARATION`] spreads from the panel.

use image::{GrayImage, Luma};
use imageproc::region_labelling::{Connectivity, connected_components};

use super::cluster::{self, Clusters};
use super::ink::{MIN_INK_SEPARATION, PANEL_SHARE, Selection, without_blends};

/// Least share of the panel's pixels inside the quad held by its largest piece.
const PANEL_SOLIDITY: f64 = 0.8;

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
        unwrapped: vec![false; clusters.labels.len()],
        score,
        panel: Some(extent),
        outlined: false,
    })
}
