//! Completion of a segmented erase mask.
//!
//! **Role:** join the ink-coloured pieces touching the segmented ink near the window to the
//! mask, refit a loose box's lettering area to the ink, dilate the mask over anti-aliased edges,
//! and refuse writing whose ink-coloured strokes the mask would still leave inside the quad.
//! **Position:** the last part of stroke segmentation, once the ink and its style are known.
//! **Signals and state:** pure functions of the keyframe plate crop; the counts go to the
//! caller's [`Trace`].
//! **Invariants:** a pixel is ink-coloured when its nearest colour cluster is ink and it lies
//! within [`INK_DELTA_E`] of the measured fill or outline; only 8-connected pieces of such pixels
//! that touch the ink, end within half a line of the analysis window and stay small beside the
//! ink join; pieces running on past that region are picture and neither join nor count as
//! strays; the refitted area lies inside the analysis window.

use image::{GrayImage, Luma, RgbImage};
use imageproc::distance_transform::Norm;
use imageproc::morphology::dilate_mut;
use imageproc::region_labelling::{Connectivity, connected_components};
use job_model::onscreen::{LetteringStyle, Point, Quad};

use super::cluster::{self, Clusters, Lab};
use super::probe::{Completeness, Trace};
use super::segment::UNSEPARATED;
use super::select::Areas;
use crate::onscreen_text::geometry;

/// Why writing whose strokes the mask would leave stays in the subtitle file.
pub(super) const STRAY: &str = "Japanese strokes reach outside the erase area";
/// How near the measured fill or outline colour an ink pixel lies, in CIE76 ΔE.
pub(crate) const INK_DELTA_E: f32 = 12.0;
/// How far past the analysis window ink may join the mask, as a share of a line.
const GROWTH_SHARE: f64 = 0.5;
/// Most ink-coloured pixels the mask may leave inside the quad, as a share of the mask's area.
const MAX_STRAY_SHARE: f64 = 0.08;
/// Least share of a loose box the ink's bounds cover: Claude's boxes are loose but lie around
/// the writing, so ink filling less of one is picture or only part of the writing.
const MIN_BOX_SHARE: f64 = 0.5;
/// Largest piece that joins the mask, as a share of the segmented ink's area: a clipped stroke's
/// end is small beside the writing, a same-coloured part of the picture is not. A joining piece
/// also holds at most a stroke's width (outline included) along a whole line height.
const MAX_JOIN_SHARE: f64 = 0.25;

/// What counts as ink colour: the chosen partition's ink clusters and the measured style.
pub(super) struct InkColours<'a> {
    pub clusters: &'a Clusters,
    /// Per cluster: erased where it forms ink pieces.
    pub ink: &'a [bool],
    pub style: &'a LetteringStyle,
}

/// The finished erase mask and, for a loose box, the lettering area.
pub(super) struct Completed {
    /// Plate-sized, 255 where strokes are erased, dilated.
    pub mask: GrayImage,
    pub lettering: Option<Quad>,
}

/// Grow `mask` (plate-sized, undilated ink) over connected ink-coloured pixels, refit the
/// lettering area when `areas.refit`, and dilate by `radius`; the stray-stroke reason when
/// ink-coloured pixels inside the quad and furigana stay outside the dilated mask beyond
/// [`MAX_STRAY_SHARE`] of its area.
pub(super) fn complete(
    plate: &RgbImage,
    mut mask: GrayImage,
    areas: &Areas,
    colours: &InkColours<'_>,
    radius: u8,
    trace: &mut Trace,
) -> Result<Completed, &'static str> {
    let (pw, ph) = plate.dimensions();
    let (ax, ay) = (
        areas.analysis.x - areas.plate.x,
        areas.analysis.y - areas.plate.y,
    );
    let reach = (GROWTH_SHARE * areas.line_height).ceil().max(0.0) as u32;
    let region = (
        ax.saturating_sub(reach),
        ay.saturating_sub(reach),
        (ax + areas.analysis.width + reach).min(pw),
        (ay + areas.analysis.height + reach).min(ph),
    );
    let coloured = ink_coloured(plate, region, colours);
    let pieces = Pieces::find(&mask, &coloured, region);
    let ink_area = mask.pixels().filter(|p| p.0[0] > 0).count();
    let style = colours.style;
    let stroke_end = (style.stroke_px + 2.0 * style.outline_px) * areas.line_height;
    let largest_join = (MAX_JOIN_SHARE * ink_area as f64).min(stroke_end) as usize;
    let mut joined = 0;
    for (x, y, id) in pieces.pixels() {
        if pieces.joins(id, largest_join) {
            mask.put_pixel(x, y, Luma([255]));
            joined += 1;
        }
    }
    let lettering = areas.refit.then(|| ink_bounds(&mask, areas)).flatten();
    if areas.refit && lettering.is_none_or(|l| box_share(areas.quad, l) < MIN_BOX_SHARE) {
        return Err(UNSEPARATED);
    }
    dilate_mut(&mut mask, Norm::L2, radius);
    let quads: Vec<Quad> = std::iter::once(areas.quad)
        .chain(areas.ruby.iter().copied())
        .collect();
    let mut stray = 0usize;
    for (x, y, id) in pieces.pixels() {
        let centre = Point {
            x: f64::from(areas.plate.x + x) + 0.5,
            y: f64::from(areas.plate.y + y) + 0.5,
        };
        if pieces.strays(id)
            && mask.get_pixel(x, y).0[0] == 0
            && quads.iter().any(|&q| geometry::contains(q, centre))
        {
            stray += 1;
        }
    }
    let mask_area = mask.pixels().filter(|p| p.0[0] > 0).count();
    trace.completeness = Some(Completeness {
        joined,
        stray,
        mask_area,
    });
    if stray as f64 > MAX_STRAY_SHARE * mask_area as f64 {
        return Err(STRAY);
    }
    Ok(Completed { mask, lettering })
}

/// Per plate pixel, whether it lies in `region` (left, top, right, bottom; exclusive) and has
/// the writing's colour: its nearest cluster is ink and it lies within [`INK_DELTA_E`] of the
/// measured fill or outline.
fn ink_coloured(
    plate: &RgbImage,
    (left, top, right, bottom): (u32, u32, u32, u32),
    colours: &InkColours<'_>,
) -> Vec<bool> {
    let (pw, ph) = plate.dimensions();
    let targets: Vec<Lab> = std::iter::once(colours.style.fill_rgb)
        .chain(colours.style.outline_rgb)
        .map(cluster::lab)
        .collect();
    let mut coloured = vec![false; (pw * ph) as usize];
    for y in top..bottom {
        for x in left..right {
            let lab = cluster::lab(plate.get_pixel(x, y).0);
            let nearest = usize::from(cluster::nearest_centre(lab, &colours.clusters.centres));
            coloured[(y * pw + x) as usize] = colours.ink[nearest]
                && targets
                    .iter()
                    .any(|&t| cluster::distance(lab, t) <= INK_DELTA_E);
        }
    }
    coloured
}

/// The 8-connected pieces of ink-coloured pixels outside the mask within the growth region.
struct Pieces {
    /// Region offset in the plate and its size.
    region: (u32, u32, u32, u32),
    /// Per region pixel, its piece id (0 for none).
    ids: Vec<usize>,
    sizes: Vec<usize>,
    /// Per piece: it reaches the region's border, so it runs on into the picture.
    open: Vec<bool>,
    /// Per piece: it touches the mask.
    attached: Vec<bool>,
}

impl Pieces {
    fn find(mask: &GrayImage, coloured: &[bool], (l, t, r, b): (u32, u32, u32, u32)) -> Pieces {
        let (pw, _) = mask.dimensions();
        let (w, h) = (r.saturating_sub(l), b.saturating_sub(t));
        let free = |x: u32, y: u32| {
            coloured[((t + y) * pw + l + x) as usize] && mask.get_pixel(l + x, t + y).0[0] == 0
        };
        let codes = GrayImage::from_fn(w, h, |x, y| Luma([u8::from(free(x, y))]));
        let labels = connected_components(&codes, Connectivity::Eight, Luma([0u8]));
        let ids: Vec<usize> = labels.pixels().map(|p| p.0[0] as usize).collect();
        let count = ids.iter().copied().max().unwrap_or(0);
        let (mut sizes, mut open, mut attached) = (
            vec![0; count + 1],
            vec![false; count + 1],
            vec![false; count + 1],
        );
        for (i, &id) in ids.iter().enumerate().filter(|(_, id)| **id != 0) {
            let (x, y) = ((i as u32) % w, (i as u32) / w);
            sizes[id] += 1;
            open[id] |= x == 0 || y == 0 || x + 1 == w || y + 1 == h;
            let (px, py) = (l + x, t + y);
            attached[id] |= (py.saturating_sub(1)..=py + 1).any(|ny| {
                (px.saturating_sub(1)..=px + 1)
                    .any(|nx| mask.get_pixel_checked(nx, ny).is_some_and(|p| p.0[0] > 0))
            });
        }
        Pieces {
            region: (l, t, w, h),
            ids,
            sizes,
            open,
            attached,
        }
    }

    /// Every pixel of a piece as plate coordinates with its piece id.
    fn pixels(&self) -> impl Iterator<Item = (u32, u32, usize)> + '_ {
        let (l, t, w, _) = self.region;
        self.ids
            .iter()
            .enumerate()
            .filter(|(_, id)| **id != 0)
            .map(move |(i, &id)| (l + (i as u32) % w, t + (i as u32) / w, id))
    }

    /// Whether piece `id` joins the mask: it touches it, ends inside the region and holds at
    /// most `largest` pixels.
    fn joins(&self, id: usize, largest: usize) -> bool {
        self.attached[id] && !self.open[id] && self.sizes[id] <= largest
    }

    /// Whether piece `id` is writing apart from the mask: it neither touches the mask nor runs
    /// on past the region.
    fn strays(&self, id: usize) -> bool {
        !self.attached[id] && !self.open[id]
    }
}

/// Share of `quad`'s bounds that `ink`'s bounds cover.
fn box_share(quad: Quad, ink: Quad) -> f64 {
    let ((ql, qt, qr, qb), (il, it, ir, ib)) = (quad.bounds(), ink.bounds());
    let common = (qr.min(ir) - ql.max(il)).max(0.0) * (qb.min(ib) - qt.max(it)).max(0.0);
    common / ((qr - ql) * (qb - qt)).max(f64::MIN_POSITIVE)
}

/// The bounds of the mask's pixels inside the analysis window, as an axis-aligned quad in frame
/// pixels; `None` without any.
fn ink_bounds(mask: &GrayImage, areas: &Areas) -> Option<Quad> {
    let (ax, ay) = (
        areas.analysis.x - areas.plate.x,
        areas.analysis.y - areas.plate.y,
    );
    let (mut l, mut t, mut r, mut b) = (u32::MAX, u32::MAX, 0, 0);
    for y in ay..ay + areas.analysis.height {
        for x in ax..ax + areas.analysis.width {
            if mask.get_pixel(x, y).0[0] > 0 {
                (l, t, r, b) = (l.min(x), t.min(y), r.max(x + 1), b.max(y + 1));
            }
        }
    }
    if r <= l || b <= t {
        return None;
    }
    let corner = |x: u32, y: u32| Point {
        x: f64::from(areas.plate.x + x),
        y: f64::from(areas.plate.y + y),
    };
    Some(Quad([
        corner(l, t),
        corner(r, t),
        corner(r, b),
        corner(l, b),
    ]))
}

#[cfg(test)]
#[path = "tests/complete.rs"]
mod tests;
