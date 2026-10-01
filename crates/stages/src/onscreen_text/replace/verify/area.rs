//! Where a replacement's lettering sits in a sampled frame, the region read around it, and which
//! found lines belong to it.
//!
//! **Role:** place the keyframe lettering quad and its furigana on the plate covering a frame, at
//! the frame's own shift, grow them into an even region of the frame to decode and read, pick
//! the upscale that makes a line tall enough to read, and tell which lines found lie over the
//! lettering (they are read as its English) and which over the original writing's own place
//! (Japanese there was not erased).
//! **Position:** between sampling and the OCR in `verify`; the placement is composition's own
//! (`compose::plate_quad_at`), so the area is where the English was lettered.
//! **Signals and state:** pure geometry.
//! **Invariants:** the region lies inside the frame on even coordinates; the scale is at least 1
//! and keeps the scaled region within `MAX_READ_PIXELS`; the writing's place reaches above the
//! lettering, where its own furigana sits, and below it only for a full-height line, never for
//! the next line's furigana.

use job_model::onscreen::{PixelRect, Point, Quad, ReplacedText, TextOccurrence};

use crate::localize::motion::Motion;
use crate::localize::still::even_region;
use crate::onscreen_text::replace::compose::{keyframe_frame, plate_at, plate_quad_at};

/// How far the region read reaches past the lettering, in lines.
pub const REGION_GROWTH_LINES: f64 = 0.75;
/// How far past the lettering a found line may reach and still count as its, in lines.
pub const KEEP_GROWTH_LINES: f64 = 0.25;
/// How far above the lettering the writing's own furigana may sit, in lines.
pub const RUBY_REACH_LINES: f64 = 0.75;
/// The least height, in lines, of a found line that is the writing's own rather than furigana.
pub const FULL_LINE_SHARE: f64 = 0.6;
/// Share of a found line's box that must lie in a place for it to count there.
pub const MIN_KEEP_SHARE: f64 = 0.5;
/// The line height the region is upscaled to, in pixels.
pub const READ_LINE_PX: f64 = 48.0;
/// The most pixels of an upscaled region.
pub const MAX_READ_PIXELS: f64 = 8_000_000.0;

/// Left, top, right, bottom.
pub type Bounds = (f64, f64, f64, f64);

/// What one sample reads.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleArea {
    /// The part of the frame decoded and read.
    pub region: PixelRect,
    /// The lettering and its furigana in source pixels, not grown.
    pub lettering: Bounds,
    /// One line of the writing, in source pixels.
    pub line_px: f64,
    /// The factor the region is enlarged by before it is read.
    pub scale: f64,
}

/// Where `text` is lettered at its keyframe: its measured lettering area, else its tracked quad.
pub fn keyframe_quad(text: &ReplacedText, occurrence: &TextOccurrence) -> Option<Quad> {
    text.lettering_quad
        .filter(|quad| quad.valid())
        .or_else(|| keyframe_frame(occurrence).map(|frame| frame.quad))
}

/// The area read of `text` at `frame` of a `size` frame, when it has a keyframe quad and a plate;
/// the lettering sits at the frame's shift in `motion`, else at its plate's own.
pub fn sample_area(
    text: &ReplacedText,
    occurrence: &TextOccurrence,
    frame: u64,
    size: (u32, u32),
    motion: &Motion,
) -> Option<SampleArea> {
    let quad = keyframe_quad(text, occurrence)?;
    let index = plate_at(&text.plates, frame)?;
    let plate = &text.plates[index];
    let shift = motion
        .shift_at(&text.id, index, frame)
        .unwrap_or(plate.shift);
    let on_frame = |quad: Quad| {
        let placed = plate_quad_at(quad, plate, shift);
        let (dx, dy) = (f64::from(plate.rect.x), f64::from(plate.rect.y));
        Quad(placed.0.map(|p| Point {
            x: p.x + dx,
            y: p.y + dy,
        }))
        .bounds()
    };
    let lettering = std::iter::once(quad)
        .chain(occurrence.ruby.iter().copied().filter(|q| q.valid()))
        .map(on_frame)
        .reduce(union)?;
    let line_px = text
        .style
        .as_ref()
        .map(|style| style.line_height_px)
        .filter(|line| line.is_finite() && *line > 0.0)
        .unwrap_or(lettering.3 - lettering.1)
        .max(1.0);
    let grown = grow(lettering, REGION_GROWTH_LINES * line_px);
    let clip = |value: f64, limit: u32| value.clamp(0.0, f64::from(limit));
    let (left, top) = (clip(grown.0, size.0).floor(), clip(grown.1, size.1).floor());
    let (right, bottom) = (clip(grown.2, size.0).ceil(), clip(grown.3, size.1).ceil());
    if right <= left || bottom <= top {
        return None;
    }
    let region = even_region(
        PixelRect {
            x: left as u32,
            y: top as u32,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        },
        size,
    )?;
    let pixels = f64::from(region.width) * f64::from(region.height);
    let limit = (MAX_READ_PIXELS / pixels).sqrt().max(1.0);
    let scale = (READ_LINE_PX / line_px).clamp(1.0, limit);
    Some(SampleArea {
        region,
        lettering,
        line_px,
        scale,
    })
}

/// Whether a line found at `found` in the upscaled region lies mostly over the lettering, grown by
/// `KEEP_GROWTH_LINES`: its reading is part of the English read back.
pub fn over_lettering(area: &SampleArea, found: Quad) -> bool {
    let target = grow(area.lettering, KEEP_GROWTH_LINES * area.line_px);
    share_inside(area, found, target) >= MIN_KEEP_SHARE
}

/// Whether a line found at `found` lies mostly where the original writing was: the lettering
/// grown by `KEEP_GROWTH_LINES` at the sides and reaching `RUBY_REACH_LINES` above it, where
/// the writing's own furigana sits; or, when it is at least `FULL_LINE_SHARE` of a line tall,
/// over the lettering grown by `KEEP_GROWTH_LINES` on every side, as the writing's own line is
/// when its erase missed it. Small writing just below the lettering is the next line's furigana.
/// Japanese read there was not erased.
pub fn over_writing(area: &SampleArea, found: Quad) -> bool {
    let (left, top, right, bottom) = area.lettering;
    let side = KEEP_GROWTH_LINES * area.line_px;
    let target = (
        left - side,
        top - RUBY_REACH_LINES * area.line_px,
        right + side,
        bottom,
    );
    let (_, box_top, _, box_bottom) = found.bounds();
    let full_line = (box_bottom - box_top) / area.scale >= FULL_LINE_SHARE * area.line_px;
    share_inside(area, found, target) >= MIN_KEEP_SHARE
        || (full_line && over_lettering(area, found))
}

/// The share of `found`'s box, in upscaled region pixels, inside `target`, in source pixels.
fn share_inside(area: &SampleArea, found: Quad, target: Bounds) -> f64 {
    let (x0, y0) = (f64::from(area.region.x), f64::from(area.region.y));
    let (l, t, r, b) = found.bounds();
    let on_frame = (
        l / area.scale + x0,
        t / area.scale + y0,
        r / area.scale + x0,
        b / area.scale + y0,
    );
    let width = (on_frame.2.min(target.2) - on_frame.0.max(target.0)).max(0.0);
    let height = (on_frame.3.min(target.3) - on_frame.1.max(target.1)).max(0.0);
    let box_area = (on_frame.2 - on_frame.0) * (on_frame.3 - on_frame.1);
    if box_area > 0.0 {
        width * height / box_area
    } else {
        0.0
    }
}

/// Found lines in reading order: rows top to bottom, each left to right; a line joins a row when
/// its centre is within half the row's first line's height of that line's centre.
pub fn reading_order(found: &mut [(Quad, f64)]) {
    found.sort_by(|a, b| a.0.center().y.total_cmp(&b.0.center().y));
    let mut start = 0;
    while start < found.len() {
        let (_, top, _, bottom) = found[start].0.bounds();
        let centre = found[start].0.center().y;
        let reach = (bottom - top) / 2.0;
        let mut end = start + 1;
        while end < found.len() && (found[end].0.center().y - centre).abs() <= reach {
            end += 1;
        }
        found[start..end].sort_by(|a, b| a.0.center().x.total_cmp(&b.0.center().x));
        start = end;
    }
}

fn union(a: Bounds, b: Bounds) -> Bounds {
    (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
}

fn grow(bounds: Bounds, by: f64) -> Bounds {
    (bounds.0 - by, bounds.1 - by, bounds.2 + by, bounds.3 + by)
}

#[cfg(test)]
#[path = "tests/area.rs"]
mod tests;
