//! Ink of the writing that lies outside its analysis window.
//!
//! **Role:** find the plate pixels of strokes the detector box clips, and the small outlined
//! reading aids (furigana) printed just above a line, so the erase mask covers them too.
//! **Position:** called by stroke segmentation once the window's ink and lettering colours are
//! known.
//! **Signals and state:** pure functions of the keyframe plate crop and the window's clusters.
//! **Invariants:** nothing returned touches the plate's border; clipped strokes reach inside the
//! window and stay within the given reach of it; furigana are only taken for outlined lettering,
//! lie over the window's columns, and are small pieces of fill wrapped in outline.

use image::{GrayImage, Luma, RgbImage};
use imageproc::distance_transform::Norm;
use imageproc::morphology::dilate;
use imageproc::region_labelling::{Connectivity, connected_components};

use super::cluster::{self, Clusters};
use super::segment::in_ring;

/// Height of the band above a line searched for furigana, as a share of the line height.
const FURIGANA_BAND: f64 = 0.6;
/// Tallest furigana piece, as a share of the line height.
const FURIGANA_HEIGHT: f64 = 0.5;
/// Least pixels of fill in a furigana piece.
const FURIGANA_MIN_FILL: usize = 4;
/// Least share of a fill-coloured blob within [`WRAP_PX`] of the outline for it to be lettering:
/// fill that runs on into a larger region is picture.
const FURIGANA_WRAPPED: f64 = 0.9;
/// How near outline pixels wrap fill pixels, in pixels (chessboard).
const WRAP_PX: u8 = 2;
/// A band pixel takes a lettering colour only within this many spreads of its centre, plus one.
const COLOUR_FIT: f32 = 3.0;

/// The analysis window inside the plate: its offset and size.
pub(super) type Placement = (u32, u32, u32, u32);

/// Plate pixels of ink strokes that the analysis window clips: 8-connected pieces of one `ink`
/// colour across the whole plate, classified by the window's centres, that reach inside the
/// window's border ring, lie no more outside the window than inside it, stay within `reach`
/// pixels of the window and clear of the plate's border, and hold at least `least` pixels.
pub(super) fn clipped_strokes(
    plate: &RgbImage,
    clusters: &Clusters,
    ink: &[bool],
    (ox, oy, w, h): Placement,
    reach: f64,
    least: usize,
) -> Vec<(u32, u32)> {
    let (pw, ph) = plate.dimensions();
    let colours = GrayImage::from_fn(pw, ph, |x, y| {
        let lab = cluster::lab(plate.get_pixel(x, y).0);
        let label = cluster::nearest_centre(lab, &clusters.centres);
        Luma([if ink[usize::from(label)] && fits(clusters, label, lab) {
            label + 1
        } else {
            0
        }])
    });
    let components = connected_components(&colours, Connectivity::Eight, Luma([0u8]));
    let count = components.pixels().map(|p| p.0[0]).max().unwrap_or(0) as usize;
    let (reach_x, reach_y) = (
        (f64::from(ox) - reach, f64::from(ox + w) + reach),
        (f64::from(oy) - reach, f64::from(oy + h) + reach),
    );
    let mut within = vec![0usize; count + 1];
    let mut outside = vec![0usize; count + 1];
    let mut enters = vec![false; count + 1];
    let mut rejected = vec![false; count + 1];
    for (x, y, p) in components.enumerate_pixels() {
        let id = p.0[0] as usize;
        if id == 0 {
            continue;
        }
        let (fx, fy) = (f64::from(x), f64::from(y));
        rejected[id] |= x == 0
            || y == 0
            || x + 1 == pw
            || y + 1 == ph
            || fx < reach_x.0
            || fx >= reach_x.1
            || fy < reach_y.0
            || fy >= reach_y.1;
        let (wx, wy) = (x.wrapping_sub(ox), y.wrapping_sub(oy));
        if wx < w && wy < h {
            within[id] += 1;
            enters[id] |= !in_ring(wx, wy, w, h);
        } else {
            outside[id] += 1;
        }
    }
    let kept: Vec<bool> = (0..=count)
        .map(|id| {
            id != 0
                && enters[id]
                && !rejected[id]
                && outside[id] <= within[id]
                && within[id] + outside[id] >= least
        })
        .collect();
    components
        .enumerate_pixels()
        .filter(|(_, _, p)| kept[p.0[0] as usize])
        .map(|(x, y, _)| (x, y))
        .collect()
}

/// Plate pixels of furigana over the window: in the band of `FURIGANA_BAND` line heights just
/// above it, 8-connected pieces in the `fill` and `outline` colours that stay clear
/// of the band's top, left and right edges, are at most `FURIGANA_HEIGHT` lines tall, and whose
/// fill lies almost wholly within `WRAP_PX` of their outline.
pub(super) fn furigana(
    plate: &RgbImage,
    clusters: &Clusters,
    (fill, outline): (u8, u8),
    (ox, oy, w, _): Placement,
    line_height: f64,
) -> Vec<(u32, u32)> {
    let band = (FURIGANA_BAND * line_height).ceil() as u32;
    if oy <= 1 || band == 0 {
        return Vec::new();
    }
    let top = oy.saturating_sub(band).max(1);
    let bottom = oy;
    let (bw, bh) = (w, bottom - top);
    let class = GrayImage::from_fn(bw, bh, |x, y| {
        let lab = cluster::lab(plate.get_pixel(ox + x, top + y).0);
        let nearest = cluster::nearest_centre(lab, &clusters.centres);
        Luma([if nearest == fill && fits(clusters, fill, lab) {
            1
        } else if nearest == outline && fits(clusters, outline, lab) {
            2
        } else {
            0
        }])
    });
    let wrapped = dilate(
        &GrayImage::from_fn(bw, bh, |x, y| {
            Luma([u8::from(class.get_pixel(x, y).0[0] == 2) * 255])
        }),
        Norm::LInf,
        WRAP_PX,
    );
    let fills = connected_components(
        &GrayImage::from_fn(bw, bh, |x, y| {
            Luma([u8::from(class.get_pixel(x, y).0[0] == 1) * 255])
        }),
        Connectivity::Eight,
        Luma([0u8]),
    );
    let fill_count = fills.pixels().map(|p| p.0[0]).max().unwrap_or(0) as usize;
    let (mut sizes, mut hugged) = (vec![0usize; fill_count + 1], vec![0usize; fill_count + 1]);
    for ((_, _, f), w) in fills.enumerate_pixels().zip(wrapped.pixels()) {
        sizes[f.0[0] as usize] += 1;
        hugged[f.0[0] as usize] += usize::from(w.0[0] > 0);
    }
    let enclosed: Vec<bool> = (0..=fill_count)
        .map(|id| id > 0 && hugged[id] as f64 >= FURIGANA_WRAPPED * sizes[id] as f64)
        .collect();
    let letter_fill = GrayImage::from_fn(bw, bh, |x, y| {
        Luma([u8::from(enclosed[fills.get_pixel(x, y).0[0] as usize]) * 255])
    });
    let hugging = dilate(&letter_fill, Norm::LInf, WRAP_PX);
    let lettering = GrayImage::from_fn(bw, bh, |x, y| {
        let outline = class.get_pixel(x, y).0[0] == 2 && hugging.get_pixel(x, y).0[0] > 0;
        Luma([u8::from(outline || letter_fill.get_pixel(x, y).0[0] > 0) * 255])
    });
    let components = connected_components(&lettering, Connectivity::Eight, Luma([0u8]));
    let count = components.pixels().map(|p| p.0[0]).max().unwrap_or(0) as usize;
    let mut pieces = vec![Piece::default(); count + 1];
    for (x, y, p) in components.enumerate_pixels() {
        let id = p.0[0] as usize;
        if id == 0 {
            continue;
        }
        let piece = &mut pieces[id];
        piece.top = piece.top.min(y);
        piece.bottom = piece.bottom.max(y + 1);
        piece.edge |= x == 0 || x + 1 == bw || y == 0;
        piece.fill += usize::from(class.get_pixel(x, y).0[0] == 1);
    }
    let tallest = FURIGANA_HEIGHT * line_height;
    let kept: Vec<bool> = pieces
        .iter()
        .enumerate()
        .map(|(id, piece)| {
            id > 0
                && !piece.edge
                && f64::from(piece.bottom.saturating_sub(piece.top)) <= tallest
                && piece.fill >= FURIGANA_MIN_FILL
        })
        .collect();
    components
        .enumerate_pixels()
        .filter(|(_, _, p)| kept[p.0[0] as usize])
        .map(|(x, y, _)| (ox + x, top + y))
        .collect()
}

/// Whether `lab` lies within [`COLOUR_FIT`] spreads (plus one) of cluster `c`'s centre.
fn fits(clusters: &Clusters, c: u8, lab: cluster::Lab) -> bool {
    let c = usize::from(c);
    cluster::distance(lab, clusters.centres[c]) <= COLOUR_FIT * clusters.spread[c] + 1.0
}

/// What one furigana candidate holds.
#[derive(Debug, Clone, Copy)]
struct Piece {
    top: u32,
    bottom: u32,
    edge: bool,
    fill: usize,
}

impl Default for Piece {
    fn default() -> Piece {
        Piece {
            top: u32::MAX,
            bottom: 0,
            edge: false,
            fill: 0,
        }
    }
}
