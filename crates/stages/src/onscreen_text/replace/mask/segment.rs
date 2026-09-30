//! Stroke segmentation of one occurrence's keyframe.
//!
//! **Role:** separate the writing's ink from its background inside the analysis window and turn
//! it into the dilated erase mask of the plate rectangle, with the measured lettering style.
//! **Position:** after the keyframe plate is decoded, before plates are followed and collected.
//! **Signals and state:** pure functions of one RGB crop; nothing is kept between occurrences.
//! **Invariants:** ink pieces are single-colour and either clear of the analysis window's
//! border or strokes followed across the plate by at most half a line and never to the plate's
//! border; furigana are added only for outlined lettering; a panel the writing sits on is never
//! erased; implausible coverage, writing the box cuts off and ink without a glyph-sized piece
//! fall back instead of guessing.

use image::{GrayImage, Luma, RgbImage};
use imageproc::distance_transform::Norm;
use imageproc::morphology::dilate_mut;
use imageproc::region_labelling::{Connectivity, connected_components};
use job_model::onscreen::{LetteringStyle, PixelRect, Point, Quad};

use super::cluster::{self, Clusters, Lab};
use super::ink::{self, Selection};
use super::reach;
use super::style;
use crate::onscreen_text::geometry;

/// Why writing that cannot be isolated stays in the subtitle file.
pub(super) const UNSEPARATED: &str = "The writing could not be separated from its background";
/// Width of the analysis window's border ring that defines the background, in pixels.
pub(super) const RING: u32 = 3;
/// Least and most share of the quad that ink may cover.
const MIN_COVERAGE: f64 = 0.01;
const MAX_COVERAGE: f64 = 0.55;
/// Most share of the ink inside the quad that may belong to pieces the window border cuts and
/// that cannot be followed to their end: beyond it the box holds only part of the writing.
const MAX_CUT_SHARE: f64 = 0.2;
/// How deep inside the window a cut piece counts: this share of a line, at least twice the ring.
const CUT_DEPTH_SHARE: f64 = 0.1;
/// Least size of the largest ink piece, as a share of the line height: writing has at least one
/// glyph-sized stroke, specks do not.
const MIN_GLYPH_SHARE: f64 = 0.3;
/// Least share of the cleanest partition's separation a partition keeps to compete on coverage.
const CLEAN_ENOUGH: f32 = 0.9;
/// Smallest ink component kept: this many pixels, or this share of the window if larger.
const MIN_COMPONENT_PX: usize = 4;
const MIN_COMPONENT_SHARE: f64 = 0.0005;
/// How far a stroke may continue past the analysis window, as a share of a line and at least
/// this many pixels: detector boxes often clip the ends of brush strokes.
const STROKE_REACH_SHARE: f64 = 0.5;
const MIN_STROKE_REACH_PX: f64 = 8.0;
/// Dilation over anti-aliased edges: at least this many pixels, else this share of a line.
const MIN_DILATION_PX: f64 = 2.0;
const DILATION_SHARE: f64 = 0.06;

/// The erase mask of the plate rectangle and the style of the writing it erases.
#[derive(Debug, Clone)]
pub(super) struct Segmentation {
    /// Plate-sized, 255 where strokes are erased.
    pub mask: GrayImage,
    pub style: LetteringStyle,
}

/// The analysis window's pixels grouped into colour clusters, with the chosen ink.
pub(super) struct Window<'a> {
    pub pixels: &'a RgbImage,
    pub lab: &'a [Lab],
    pub clusters: &'a Clusters,
    /// Per cluster: background, never erased.
    pub background: &'a [bool],
    /// Per cluster: a colour the writing is drawn in, fill or outline.
    pub core: &'a [bool],
    /// Ink pixels, row-major: pieces clear of the border and strokes followed past it.
    pub ink: &'a [bool],
}

/// One partition's ink that passed every guard.
struct Choice<'a> {
    clusters: &'a Clusters,
    selection: Selection,
    /// Window ink: pieces clear of the border and strokes followed past it.
    ink: Vec<bool>,
    /// Plate pixels of strokes the window clips.
    clipped: Vec<(u32, u32)>,
    /// Ink pixels inside the quad, kept or followed past the border.
    covered: usize,
}

/// The analysis window inside the plate and the sizes that scale its guards.
struct Frame<'a> {
    plate: &'a RgbImage,
    /// Offset of the window in the plate and its size.
    placement: reach::Placement,
    /// Per window pixel, whether it lies inside the quad.
    inside: Vec<bool>,
    least: usize,
    line_height: f64,
}

/// Segment the writing in `window` of the keyframe `plate` crop taken at `plate_rect`.
///
/// Every partition of two to five colours is read as lettering over the ring's background, and
/// [`best`] picks among those that pass the guards. When none does, the partitions are read as
/// writing printed on a panel filling the box.
pub(super) fn segment(
    plate: &RgbImage,
    plate_rect: PixelRect,
    window: PixelRect,
    quad: Quad,
    line_height: f64,
) -> Result<Segmentation, &'static str> {
    let (ox, oy) = (window.x - plate_rect.x, window.y - plate_rect.y);
    let (w, h) = (window.width, window.height);
    if w <= 2 * RING + 1 || h <= 2 * RING + 1 {
        return Err(UNSEPARATED);
    }
    let crop = image::imageops::crop_imm(plate, ox, oy, w, h).to_image();
    let lab: Vec<Lab> = crop.pixels().map(|p| cluster::lab(p.0)).collect();
    let partitions = cluster::partitions(&lab);
    let radius = dilation_radius(line_height);
    let frame = Frame {
        plate,
        placement: (ox, oy, w, h),
        inside: in_quad(window, quad),
        least: MIN_COMPONENT_PX.max((MIN_COMPONENT_SHARE * f64::from(w * h)).ceil() as usize),
        line_height,
    };
    let chosen = best(partitions.iter().filter_map(|clusters| {
        ink::lettering(clusters, w, h, &frame.inside, radius)
            .and_then(|s| judge(&frame, clusters, s))
    }))
    .or_else(|| {
        best(partitions.iter().filter_map(|clusters| {
            ink::panel(clusters, w, h, &frame.inside).and_then(|s| judge(&frame, clusters, s))
        }))
    })
    .ok_or(UNSEPARATED)?;
    let view = Window {
        pixels: &crop,
        lab: &lab,
        clusters: chosen.clusters,
        background: &chosen.selection.background,
        core: &chosen.selection.core,
        ink: &chosen.ink,
    };
    let style = style::measure(&view, line_height);
    let furigana = match style::roles(&view) {
        (fill, Some(outline)) if chosen.selection.panel.is_none() => reach::furigana(
            plate,
            chosen.clusters,
            (fill, outline),
            frame.placement,
            line_height,
        ),
        _ => Vec::new(),
    };
    let mut mask = GrayImage::new(plate_rect.width, plate_rect.height);
    for (i, _) in chosen.ink.iter().enumerate().filter(|(_, ink)| **ink) {
        let (x, y) = ((i as u32) % w, (i as u32) / w);
        mask.put_pixel(ox + x, oy + y, Luma([255]));
    }
    for &(x, y) in chosen.clipped.iter().chain(&furigana) {
        mask.put_pixel(x, y, Luma([255]));
    }
    dilate_mut(&mut mask, Norm::L2, radius);
    Ok(Segmentation { mask, style })
}

/// Pixels of the window's outer border ring.
pub(super) fn in_ring(x: u32, y: u32, width: u32, height: u32) -> bool {
    x < RING || y < RING || x + RING >= width || y + RING >= height
}

/// How far the mask grows past the detected ink, in pixels.
pub(super) fn dilation_radius(line_height: f64) -> u8 {
    MIN_DILATION_PX
        .max((DILATION_SHARE * line_height).round())
        .min(f64::from(u8::MAX)) as u8
}

/// Of the choices whose split is nearly as clean as the cleanest, the one whose ink covers most
/// of the quad; the first on ties. A cleaner split keeps picture colours out of the ink, and more
/// coverage keeps the writing's own edges in it.
fn best<'a>(choices: impl Iterator<Item = Choice<'a>>) -> Option<Choice<'a>> {
    let choices: Vec<Choice<'a>> = choices.collect();
    let cleanest = choices
        .iter()
        .map(|c| c.selection.score)
        .fold(f32::NEG_INFINITY, f32::max);
    choices
        .into_iter()
        .filter(|c| c.selection.score >= CLEAN_ENOUGH * cleanest)
        .fold(None, |best: Option<Choice<'a>>, next| match best {
            Some(b) if b.covered >= next.covered => Some(b),
            _ => Some(next),
        })
}

/// The selection's kept ink and clipped strokes, when they cover a plausible share of the quad,
/// the box does not cut most of the writing off and one piece is glyph-sized.
fn judge<'a>(
    frame: &Frame<'_>,
    clusters: &'a Clusters,
    selection: Selection,
) -> Option<Choice<'a>> {
    let (ox, oy, w, h) = frame.placement;
    let eligible: Vec<u8> = clusters
        .labels
        .iter()
        .zip(&selection.pixels)
        .map(|(&l, &p)| if p { l + 1 } else { 0 })
        .collect();
    let kept = kept_ink(&eligible, w, h, frame.least);
    let clipped = reach::clipped_strokes(
        frame.plate,
        clusters,
        &selection.ink,
        frame.placement,
        MIN_STROKE_REACH_PX.max(STROKE_REACH_SHARE * frame.line_height),
        frame.least,
    );
    let mut ink = kept;
    for &(x, y) in &clipped {
        let (wx, wy) = (x.wrapping_sub(ox), y.wrapping_sub(oy));
        if wx < w && wy < h {
            ink[(wy * w + wx) as usize] = true;
        }
    }
    let quad = frame.inside.iter().filter(|&&i| i).count();
    let covered = count_inside(&ink, &frame.inside);
    let depth = (2 * RING).max((CUT_DEPTH_SHARE * frame.line_height).round() as u32);
    let on_panel = |i: usize| {
        let (x, y) = ((i as u32) % w, (i as u32) / w);
        selection
            .panel
            .is_none_or(|(l, t, r, b)| x >= l && x < r && y >= t && y < b)
    };
    let cut = cut_pieces(&eligible, &ink, (w, h), frame.least, depth)
        .iter()
        .zip(&frame.inside)
        .enumerate()
        .filter(|(i, (c, q))| **c && **q && on_panel(*i))
        .count();
    let share = covered as f64 / quad.max(1) as f64;
    let plausible = quad > 0
        && (MIN_COVERAGE..=MAX_COVERAGE).contains(&share)
        && cut as f64 <= MAX_CUT_SHARE * (cut + covered) as f64
        && f64::from(largest_piece(&ink, w, h)) >= MIN_GLYPH_SHARE * frame.line_height;
    plausible.then_some(Choice {
        clusters,
        selection,
        ink,
        clipped,
        covered,
    })
}

fn count_inside(pixels: &[bool], inside: &[bool]) -> usize {
    pixels
        .iter()
        .zip(inside)
        .filter(|(p, i)| **p && **i)
        .count()
}

/// Pixels of single-colour ink pieces (`colours`: cluster plus one, zero for none) of at least
/// `least` pixels that the window border cuts and that reach `depth` pixels into the window,
/// other than the ink already followed past the border. Pieces the box edge only grazes are
/// picture around the writing.
fn cut_pieces(
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
fn largest_piece(pixels: &[bool], w: u32, h: u32) -> u32 {
    let codes: Vec<u8> = pixels.iter().map(|&p| u8::from(p)).collect();
    let (sizes, _, components) = pieces(&codes, w, h);
    let mut bounds = vec![(u32::MAX, u32::MAX, 0u32, 0u32); sizes.len()];
    for (i, &id) in components.iter().enumerate().filter(|(_, id)| **id != 0) {
        let (x, y) = ((i as u32) % w, (i as u32) / w);
        let b = &mut bounds[id];
        *b = (b.0.min(x), b.1.min(y), b.2.max(x + 1), b.3.max(y + 1));
    }
    bounds
        .iter()
        .skip(1)
        .map(|&(l, t, r, b)| r.saturating_sub(l).max(b.saturating_sub(t)))
        .max()
        .unwrap_or(0)
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

/// Pixels of single-colour ink pieces (`colours`: cluster plus one, zero for none) of at least
/// `least` pixels clear of the window border.
fn kept_ink(colours: &[u8], w: u32, h: u32, least: usize) -> Vec<bool> {
    let (sizes, touches, ids) = pieces(colours, w, h);
    ids.iter()
        .map(|&id| id != 0 && sizes[id] >= least && !touches[id])
        .collect()
}

/// Per window pixel, whether its centre lies inside the quad.
fn in_quad(window: PixelRect, quad: Quad) -> Vec<bool> {
    (0..window.height)
        .flat_map(|y| (0..window.width).map(move |x| (x, y)))
        .map(|(x, y)| {
            geometry::contains(
                quad,
                Point {
                    x: f64::from(window.x + x) + 0.5,
                    y: f64::from(window.y + y) + 0.5,
                },
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/segment.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/separation.rs"]
mod separation;
