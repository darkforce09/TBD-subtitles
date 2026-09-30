//! Stroke segmentation of one occurrence's keyframe.
//!
//! **Role:** separate the writing's ink from its background inside the analysis window and turn
//! it into the dilated erase mask of the plate rectangle, with the measured lettering style and,
//! for a loose Claude box, the lettering area refitted to the ink.
//! **Position:** after the keyframe plate is decoded, before plates are followed and collected.
//! **Signals and state:** pure functions of one RGB crop; every partition's figures go to the
//! caller's [`Trace`]; nothing is kept between occurrences.
//! **Invariants:** ink pieces are single-colour and either clear of the analysis window's
//! border, inside a furigana quad, or strokes followed across the plate by at most half a line
//! and never to the plate's border; frames around the writing and, for a loose box, pieces
//! mostly outside it are never ink; furigana without quads are added only for outlined
//! lettering; a panel the writing sits on is never erased; the outlined reading along the ring
//! is tried only for detector boxes after the others; implausible coverage, writing the box cuts
//! off, ink without a glyph-sized piece and ink-coloured strokes the mask leaves fall back
//! instead of guessing.

use std::collections::HashSet;

use image::{GrayImage, Luma, RgbImage};
use job_model::onscreen::{LetteringStyle, Point, Quad};

use super::cluster::{self, Clusters, Lab};
use super::complete;
use super::ink::{self, Selection};
use super::panel;
use super::pieces::{apart, cut_pieces, frames, kept_ink, largest_piece, ruby_ink};
use super::probe::{Judgement, Reading, Trace};
use super::reach;
use super::select::Areas;
use super::style;
use crate::onscreen_text::geometry;

/// Why writing that cannot be isolated stays in the subtitle file.
pub(super) const UNSEPARATED: &str = "The writing could not be separated from its background";
/// Width of the analysis window's border ring that defines the background, in pixels.
pub(super) const RING: u32 = 3;
/// Least and most share of the quad that ink may cover.
const MIN_COVERAGE: f64 = 0.01;
const MAX_COVERAGE: f64 = 0.55;
/// Most share of the quad a fill and outline wrapping each other may cover when read as dense
/// outlined lettering, which covers more than [`MAX_COVERAGE`]: bold lettering with a heavy
/// outline fills a box drawn tight around it.
const MAX_OUTLINED_COVERAGE: f64 = 0.8;
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

/// The guards a partition can fail, as the trace names them.
const NO_INK: &str = "no colour reads as ink";
const IMPLAUSIBLE_COVERAGE: &str = "ink covers too little or too much of the quad";
const CUT_OFF: &str = "the box cuts the writing off";
const NO_GLYPH: &str = "no piece is glyph-sized";

/// The erase mask of the plate rectangle and the style of the writing it erases.
#[derive(Debug, Clone)]
pub(super) struct Segmentation {
    /// Plate-sized, 255 where strokes are erased.
    pub mask: GrayImage,
    pub style: LetteringStyle,
    /// For a loose Claude box, the ink's bounds in frame pixels: where the English goes.
    pub lettering: Option<Quad>,
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
    /// Window ink: pieces clear of the border, furigana and strokes followed past it.
    ink: Vec<bool>,
    /// Plate pixels of strokes the window clips.
    clipped: Vec<(u32, u32)>,
    /// Ink pixels inside the quad and furigana, kept or followed past the border.
    covered: usize,
}

/// The analysis window inside the plate and the sizes that scale its guards.
struct Frame<'a> {
    plate: &'a RgbImage,
    /// Offset of the window in the plate and its size.
    placement: reach::Placement,
    /// Per window pixel, whether it lies inside the line's quad.
    line: Vec<bool>,
    /// Per window pixel, whether it lies inside a furigana quad.
    ruby: Vec<bool>,
    /// Per window pixel, whether it lies inside the line's quad or a furigana quad.
    inside: Vec<bool>,
    least: usize,
    line_height: f64,
    /// The quad is a loose box: ink pieces that never enter it are picture beside the writing.
    loose: bool,
}

/// Segment the writing in the analysis window of the keyframe `plate` crop taken at
/// `areas.plate`, recording every partition's figures in `trace`.
///
/// Every partition of two to five colours is read as lettering over the ring's background, and
/// [`best`] picks among those that pass the guards. When none does, or the choice holds no fill
/// and outline wrapping each other, the cleanly separated outlined ones are read again as dense
/// lettering that covers more of the quad than plain lettering may, and a passing one is chosen
/// instead. Without a choice the partitions are then read as writing printed on a panel filling
/// the box, and then, for a detector box, as outlined lettering whose colours also run along the
/// ring.
pub(super) fn segment(
    plate: &RgbImage,
    areas: &Areas,
    trace: &mut Trace,
) -> Result<Segmentation, &'static str> {
    let window = areas.analysis;
    let (ox, oy) = (window.x - areas.plate.x, window.y - areas.plate.y);
    let (w, h) = (window.width, window.height);
    if w <= 2 * RING + 1 || h <= 2 * RING + 1 {
        return Err(UNSEPARATED);
    }
    let crop = image::imageops::crop_imm(plate, ox, oy, w, h).to_image();
    let lab: Vec<Lab> = crop.pixels().map(|p| cluster::lab(p.0)).collect();
    let partitions = cluster::partitions(&lab);
    let radius = dilation_radius(areas.line_height);
    let line = in_quads(window, std::slice::from_ref(&areas.quad));
    let ruby = in_quads(window, &areas.ruby);
    let frame = Frame {
        plate,
        placement: (ox, oy, w, h),
        inside: line.iter().zip(&ruby).map(|(l, r)| *l || *r).collect(),
        line,
        ruby,
        least: MIN_COMPONENT_PX.max((MIN_COMPONENT_SHARE * f64::from(w * h)).ceil() as usize),
        line_height: areas.line_height,
        loose: areas.refit,
    };
    let mut chosen = None;
    let readings: &[Reading] = if areas.refit {
        &[Reading::Lettering, Reading::DenseOutlined, Reading::Panel]
    } else {
        &[
            Reading::Lettering,
            Reading::DenseOutlined,
            Reading::Panel,
            Reading::OutlinedOverRing,
        ]
    };
    for &reading in readings {
        let settled = chosen.as_ref().is_some_and(|(_, c): &(usize, Choice)| {
            reading != Reading::DenseOutlined || c.selection.outlined
        });
        if settled {
            break;
        }
        let mut choices = Vec::new();
        for clusters in &partitions {
            let selection = match reading {
                Reading::Lettering => ink::lettering(clusters, w, h, &frame.line, radius),
                Reading::DenseOutlined => ink::lettering(clusters, w, h, &frame.line, radius)
                    .filter(|s| s.outlined && s.score >= ink::MIN_INK_SEPARATION),
                Reading::Panel => panel::panel(clusters, w, h, &frame.line),
                Reading::OutlinedOverRing => {
                    ink::outlined_over_ring(clusters, w, h, &frame.line, radius)
                }
            };
            choices.extend(judge(&frame, clusters, selection, reading, trace));
        }
        if let Some(found) = best(choices) {
            chosen = Some(found);
        }
    }
    let (index, chosen) = chosen.ok_or(UNSEPARATED)?;
    trace.partitions[index].chosen = true;
    let view = Window {
        pixels: &crop,
        lab: &lab,
        clusters: chosen.clusters,
        background: &chosen.selection.background,
        core: &chosen.selection.core,
        ink: &chosen.ink,
    };
    let style = style::measure(&view, areas.line_height);
    let furigana = match style::roles(&view) {
        (fill, Some(outline)) if areas.ruby.is_empty() => reach::furigana(
            plate,
            chosen.clusters,
            (fill, outline),
            frame.placement,
            areas.line_height,
        ),
        _ => Vec::new(),
    };
    let mut mask = GrayImage::new(areas.plate.width, areas.plate.height);
    for (i, _) in chosen.ink.iter().enumerate().filter(|(_, ink)| **ink) {
        let (x, y) = ((i as u32) % w, (i as u32) / w);
        mask.put_pixel(ox + x, oy + y, Luma([255]));
    }
    for &(x, y) in chosen.clipped.iter().chain(&furigana) {
        mask.put_pixel(x, y, Luma([255]));
    }
    let ink_colours = complete::InkColours {
        clusters: chosen.clusters,
        ink: &chosen.selection.ink,
        style: &style,
    };
    let completed = complete::complete(plate, mask, areas, &ink_colours, radius, trace)?;
    Ok(Segmentation {
        mask: completed.mask,
        lettering: completed.lettering,
        style,
    })
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
/// coverage keeps the writing's own edges in it. Choices carry their trace index.
fn best<'a>(choices: Vec<(usize, Choice<'a>)>) -> Option<(usize, Choice<'a>)> {
    let cleanest = choices
        .iter()
        .map(|(_, c)| c.selection.score)
        .fold(f32::NEG_INFINITY, f32::max);
    choices
        .into_iter()
        .filter(|(_, c)| c.selection.score >= CLEAN_ENOUGH * cleanest)
        .fold(None, |best: Option<(usize, Choice<'a>)>, next| match best {
            Some(b) if b.1.covered >= next.1.covered => Some(b),
            _ => Some(next),
        })
}

/// The selection's kept ink, furigana and clipped strokes, when they cover a plausible share of
/// the quad, the box does not cut most of the writing off and one piece is glyph-sized. The
/// figures go to `trace`; a passing choice carries its trace index.
fn judge<'a>(
    frame: &Frame<'_>,
    clusters: &'a Clusters,
    selection: Option<Selection>,
    reading: Reading,
    trace: &mut Trace,
) -> Option<(usize, Choice<'a>)> {
    let index = trace.partitions.len();
    let mut judgement = Judgement {
        reading,
        colours: clusters.centres.len(),
        separation: None,
        coverage: 0.0,
        cut_share: 0.0,
        largest_piece: 0,
        failure: Some(NO_INK),
        chosen: false,
    };
    let Some(selection) = selection else {
        trace.partitions.push(judgement);
        return None;
    };
    let (ox, oy, w, h) = frame.placement;
    let eligible: Vec<u8> = clusters
        .labels
        .iter()
        .zip(&selection.pixels)
        .map(|(&l, &p)| if p { l + 1 } else { 0 })
        .collect();
    let mut ink = kept_ink(&eligible, w, h, frame.least);
    for (i, r) in ruby_ink(&eligible, &frame.ruby, w, h, frame.least)
        .into_iter()
        .enumerate()
    {
        ink[i] |= r;
    }
    let mut clipped = reach::clipped_strokes(
        frame.plate,
        clusters,
        &selection.ink,
        frame.placement,
        MIN_STROKE_REACH_PX.max(STROKE_REACH_SHARE * frame.line_height),
        frame.least,
    );
    for &(x, y) in &clipped {
        let (wx, wy) = (x.wrapping_sub(ox), y.wrapping_sub(oy));
        if wx < w && wy < h {
            ink[(wy * w + wx) as usize] = true;
        }
    }
    for (i, unwrapped) in selection.unwrapped.iter().enumerate() {
        ink[i] &= !unwrapped;
    }
    for (i, border) in frames(&ink, w, h, frame.line_height)
        .into_iter()
        .enumerate()
    {
        ink[i] &= !border;
    }
    if frame.loose {
        for (i, beside) in apart(&ink, &frame.line, w, h).into_iter().enumerate() {
            ink[i] &= !beside;
        }
    }
    clipped = still_attached(&clipped, &ink, frame.placement);
    let quad = frame.line.iter().filter(|&&i| i).count();
    let covered = count_inside(&ink, &frame.inside);
    let depth = (2 * RING).max((CUT_DEPTH_SHARE * frame.line_height).round() as u32);
    let on_panel = |i: usize| {
        let (x, y) = ((i as u32) % w, (i as u32) / w);
        selection
            .panel
            .is_none_or(|(l, t, r, b)| x >= l && x < r && y >= t && y < b)
    };
    let cuts = cut_pieces(&eligible, &ink, (w, h), frame.least, depth);
    let cut = cuts
        .iter()
        .zip(&frame.inside)
        .enumerate()
        .filter(|(i, (c, q))| **c && **q && on_panel(*i))
        .count();
    judgement.separation = Some(selection.score);
    judgement.coverage = count_inside(&ink, &frame.line) as f64 / quad.max(1) as f64;
    judgement.cut_share = cut as f64 / (cut + covered).max(1) as f64;
    judgement.largest_piece = largest_piece(&ink, w, h);
    let plausible = if reading == Reading::DenseOutlined {
        judgement.coverage > MAX_COVERAGE && judgement.coverage <= MAX_OUTLINED_COVERAGE
    } else {
        (MIN_COVERAGE..=MAX_COVERAGE).contains(&judgement.coverage)
    };
    judgement.failure = if quad == 0 || !plausible {
        Some(IMPLAUSIBLE_COVERAGE)
    } else if cut as f64 > MAX_CUT_SHARE * (cut + covered) as f64 {
        Some(CUT_OFF)
    } else if f64::from(judgement.largest_piece) < MIN_GLYPH_SHARE * frame.line_height {
        Some(NO_GLYPH)
    } else {
        None
    };
    let passed = judgement.failure.is_none();
    trace.partitions.push(judgement);
    passed.then_some((
        index,
        Choice {
            clusters,
            selection,
            ink,
            clipped,
            covered,
        },
    ))
}

/// The clipped stroke pixels (plate coordinates) still joined to window `ink`: those inside the
/// window that are ink, and those outside 8-connected to them through clipped pixels.
fn still_attached(
    clipped: &[(u32, u32)],
    ink: &[bool],
    (ox, oy, w, h): reach::Placement,
) -> Vec<(u32, u32)> {
    let inside = |(x, y): (u32, u32)| {
        let (wx, wy) = (x.wrapping_sub(ox), y.wrapping_sub(oy));
        (wx < w && wy < h).then(|| (wy * w + wx) as usize)
    };
    let offered: HashSet<(u32, u32)> = clipped.iter().copied().collect();
    let mut kept: HashSet<(u32, u32)> = clipped
        .iter()
        .copied()
        .filter(|&p| inside(p).is_some_and(|i| ink[i]))
        .collect();
    let mut queue: Vec<(u32, u32)> = kept.iter().copied().collect();
    while let Some((x, y)) = queue.pop() {
        for (dx, dy) in [
            (-1i64, -1i64),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ] {
            let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
            let Ok(next) = u32::try_from(nx).and_then(|nx| Ok((nx, u32::try_from(ny)?))) else {
                continue;
            };
            if offered.contains(&next) && inside(next).is_none() && kept.insert(next) {
                queue.push(next);
            }
        }
    }
    clipped
        .iter()
        .copied()
        .filter(|p| kept.contains(p))
        .collect()
}

fn count_inside(pixels: &[bool], inside: &[bool]) -> usize {
    pixels
        .iter()
        .zip(inside)
        .filter(|(p, i)| **p && **i)
        .count()
}

/// Per pixel of `window`, whether its centre lies inside any of `quads`.
fn in_quads(window: job_model::onscreen::PixelRect, quads: &[Quad]) -> Vec<bool> {
    (0..window.height)
        .flat_map(|y| (0..window.width).map(move |x| (x, y)))
        .map(|(x, y)| {
            let centre = Point {
                x: f64::from(window.x + x) + 0.5,
                y: f64::from(window.y + y) + 0.5,
            };
            quads.iter().any(|&quad| geometry::contains(quad, centre))
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/segment.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/separation.rs"]
mod separation;
