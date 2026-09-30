//! Connected ink pieces of an analysis window.
//!
//! **Role:** cut per-pixel colour codes into 8-connected pieces and answer the questions
//! segmentation asks of them: which pieces the window keeps, which it cuts, which furigana hold,
//! which fill pieces their outline does not ring, which are frames around the writing or picture
//! beside a loose box, and how large the largest is.
//! **Position:** helpers of stroke segmentation, called once per judged partition.
//! **Signals and state:** pure functions of row-major codes and the window size.
//! **Invariants:** a piece holds pixels of one non-zero code (or of the ink, for the frame and
//! loose-box questions); code zero is never a piece.

use image::{GrayImage, Luma};
use imageproc::region_labelling::{Connectivity, connected_components};

/// Pixels of single-colour ink pieces (`colours`: cluster plus one, zero for none) of at least
/// `least` pixels that the window border cuts and that reach `depth` pixels into the window,
/// other than the ink already followed past the border. Pieces the box edge only grazes are
/// picture around the writing.
pub(super) fn cut_pieces(
    colours: &[u8],
    ink: &[bool],
    (w, h): (u32, u32),
    least: usize,
    depth: u32,
) -> Vec<bool> {
    let (sizes, touches, components) = pieces(colours, w, h);
    let mut deep = vec![false; sizes.len()];
    for (i, &id) in components.iter().enumerate() {
        let (x, y) = ((i as u32) % w, (i as u32) / w);
        deep[id] |= x.min(y).min(w - 1 - x).min(h - 1 - y) >= depth;
    }
    components
        .iter()
        .zip(ink)
        .map(|(&id, &followed)| {
            id != 0 && touches[id] && deep[id] && sizes[id] >= least && !followed
        })
        .collect()
}

/// The longest bounding-box side of any 8-connected piece of `pixels`.
pub(super) fn largest_piece(pixels: &[bool], w: u32, h: u32) -> u32 {
    let (_, bounds, _) = bounded(pixels, w, h);
    bounds
        .iter()
        .skip(1)
        .map(|&(l, t, r, b)| r.saturating_sub(l).max(b.saturating_sub(t)))
        .max()
        .unwrap_or(0)
}

/// Pixels of single-colour ink pieces (`colours`: cluster plus one, zero for none) of at least
/// `least` pixels clear of the window border.
pub(super) fn kept_ink(colours: &[u8], w: u32, h: u32, least: usize) -> Vec<bool> {
    let (sizes, touches, ids) = pieces(colours, w, h);
    ids.iter()
        .map(|&id| id != 0 && sizes[id] >= least && !touches[id])
        .collect()
}

/// Pixels inside furigana quads (`ruby`) of single-colour ink pieces of at least `least`
/// pixels, whether or not the window border cuts them: furigana a line's quad clips are ink.
pub(super) fn ruby_ink(colours: &[u8], ruby: &[bool], w: u32, h: u32, least: usize) -> Vec<bool> {
    let (sizes, _, ids) = pieces(colours, w, h);
    ids.iter()
        .zip(ruby)
        .map(|(&id, &in_ruby)| in_ruby && id != 0 && sizes[id] >= least)
        .collect()
}

/// Pixels of `ink` pieces that are frames around the writing rather than writing: 8-connected
/// pieces spanning more than [`FRAME_SPAN`] line heights one way and more than [`FRAME_DEPTH`]
/// the other, covering less than [`FRAME_DENSITY`] of their bounds and enclosing at least
/// [`FRAME_ENCLOSES`] of the other ink, as a card's ruled border does. A line of writing fills
/// its bounds more densely and encloses nothing but furigana; a lone rule is thinner than a line.
pub(super) fn frames(ink: &[bool], w: u32, h: u32, line_height: f64) -> Vec<bool> {
    let (sizes, bounds, ids) = bounded(ink, w, h);
    let total: usize = sizes.iter().skip(1).sum();
    let (span, depth) = (FRAME_SPAN * line_height, FRAME_DEPTH * line_height);
    let frame: Vec<bool> = (0..sizes.len())
        .map(|id| {
            let (l, t, r, b) = bounds[id];
            let (bw, bh) = (
                f64::from(r.saturating_sub(l)),
                f64::from(b.saturating_sub(t)),
            );
            if id == 0
                || bw.max(bh) <= span
                || bw.min(bh) <= depth
                || sizes[id] as f64 >= FRAME_DENSITY * bw * bh
            {
                return false;
            }
            let enclosed = ids
                .iter()
                .enumerate()
                .filter(|&(i, &other)| {
                    let (x, y) = ((i as u32) % w, (i as u32) / w);
                    other != 0 && other != id && x >= l && x < r && y >= t && y < b
                })
                .count();
            enclosed as f64 >= FRAME_ENCLOSES * (total - sizes[id]) as f64
        })
        .collect();
    ids.iter().map(|&id| frame[id]).collect()
}

/// A frame spans more than this many line heights one way.
const FRAME_SPAN: f64 = 2.5;
/// A frame spans more than this many line heights the other way.
const FRAME_DEPTH: f64 = 0.8;
/// A frame covers less than this share of its bounds.
const FRAME_DENSITY: f64 = 0.35;
/// A frame's bounds hold at least this share of the other ink.
const FRAME_ENCLOSES: f64 = 0.5;

/// Pixels of `ink` pieces with less than [`LEAST_INSIDE`] of their pixels `inside`: picture or
/// neighbouring writing a padded window takes in beside the writing a loose box marks.
pub(super) fn apart(ink: &[bool], inside: &[bool], w: u32, h: u32) -> Vec<bool> {
    let (sizes, _, ids) = bounded(ink, w, h);
    let mut within = vec![0usize; sizes.len()];
    for (&id, &i) in ids.iter().zip(inside) {
        within[id] += usize::from(i);
    }
    ids.iter()
        .map(|&id| id != 0 && (within[id] as f64) < LEAST_INSIDE * sizes[id] as f64)
        .collect()
}

/// Least share of a piece's pixels inside a loose box for the piece to be the box's writing.
const LEAST_INSIDE: f64 = 1.0 / 3.0;

/// Pixels of 8-connected pieces of `fill` whose edge lies less than [`LEAST_WRAPPED_EDGE`]
/// within `near_outline`: an outlined glyph's fill is ringed by its outline, while a patch of
/// picture in the fill's colour borders other colours.
pub(super) fn unwrapped(fill: &[bool], near_outline: &[bool], w: u32, h: u32) -> Vec<bool> {
    let (sizes, _, ids) = bounded(fill, w, h);
    let (mut edge, mut wrapped) = (vec![0usize; sizes.len()], vec![0usize; sizes.len()]);
    for (i, &id) in ids.iter().enumerate().filter(|(_, id)| **id != 0) {
        let (x, y) = ((i as u32) % w, (i as u32) / w);
        let apart = |nx: u32, ny: u32| !fill[(ny * w + nx) as usize];
        let on_edge = (x > 0 && apart(x - 1, y))
            || (x + 1 < w && apart(x + 1, y))
            || (y > 0 && apart(x, y - 1))
            || (y + 1 < h && apart(x, y + 1));
        if on_edge {
            edge[id] += 1;
            wrapped[id] += usize::from(near_outline[i]);
        }
    }
    ids.iter()
        .map(|&id| id != 0 && (wrapped[id] as f64) < LEAST_WRAPPED_EDGE * edge[id] as f64)
        .collect()
}

/// Least share of a fill piece's edge within reach of its outline.
const LEAST_WRAPPED_EDGE: f64 = 0.8;

/// A piece's bounds as (left, top, right, bottom), exclusive right and bottom.
type Bounds = (u32, u32, u32, u32);

/// 8-connected pieces of `pixels`: each piece's size and bounds, indexed by piece id, and every
/// pixel's piece id (0 for none).
fn bounded(pixels: &[bool], w: u32, h: u32) -> (Vec<usize>, Vec<Bounds>, Vec<usize>) {
    let codes: Vec<u8> = pixels.iter().map(|&p| u8::from(p)).collect();
    let (sizes, _, ids) = pieces(&codes, w, h);
    let mut bounds = vec![(u32::MAX, u32::MAX, 0u32, 0u32); sizes.len()];
    for (i, &id) in ids.iter().enumerate().filter(|(_, id)| **id != 0) {
        let (x, y) = ((i as u32) % w, (i as u32) / w);
        let b = &mut bounds[id];
        *b = (b.0.min(x), b.1.min(y), b.2.max(x + 1), b.3.max(y + 1));
    }
    (sizes, bounds, ids)
}

/// 8-connected pieces of equal non-zero `codes`: each piece's size and whether it touches the
/// window border, indexed by piece id, and every pixel's piece id (0 for none).
fn pieces(codes: &[u8], w: u32, h: u32) -> (Vec<usize>, Vec<bool>, Vec<usize>) {
    let image = GrayImage::from_fn(w, h, |x, y| Luma([codes[(y * w + x) as usize]]));
    let components = connected_components(&image, Connectivity::Eight, Luma([0u8]));
    let count = components.pixels().map(|p| p.0[0]).max().unwrap_or(0) as usize;
    let mut sizes = vec![0usize; count + 1];
    let mut touches = vec![false; count + 1];
    let ids: Vec<usize> = components.pixels().map(|p| p.0[0] as usize).collect();
    for (i, &id) in ids.iter().enumerate() {
        let (x, y) = ((i as u32) % w, (i as u32) / w);
        sizes[id] += 1;
        touches[id] |= x == 0 || y == 0 || x + 1 == w || y + 1 == h;
    }
    (sizes, touches, ids)
}

#[cfg(test)]
#[path = "tests/pieces.rs"]
mod tests;
