//! Japanese strokes an inpainted plate still shows.
//!
//! **Role:** measure how much of a filled plate's erased area still looks like the original
//! lettering: pixels near the measured fill colour that stand out from the fill around them.
//! **Position:** called by the inpainting step on every filled plate of an occurrence with a
//! measured style, before the plate is recorded; decides the one wider retry. Whether strokes
//! that survive the retry leave the replacement unclean is the read-back check's call.
//! **Signals and state:** pure functions of one filled plate, its mask and the lettering style.
//! **Invariants:** deterministic; reads only plate pixels under the mask and their neighbourhood;
//! the neighbourhood median is sampled on a fixed grid, so the cost per masked pixel is bounded.

use image::{GrayImage, Luma, RgbImage};
use imageproc::distance_transform::Norm;
use imageproc::morphology::{dilate, open};
use job_model::onscreen::LetteringStyle;

use crate::onscreen_text::replace::mask::{INK_DELTA_E, delta_e};

/// Most share of the mask's pixels that may still look like the lettering after the fill.
pub const MAX_RESIDUE_SHARE: f64 = 0.03;
/// How far a remaining stroke pixel stands out from the fill's local median, in CIE76 ΔE: a fill
/// that is itself near the lettering colour (a white sign under white writing) is not a stroke.
const STANDS_OUT_DELTA_E: f32 = 20.0;
/// Radius of the neighbourhood whose median a pixel is compared with, as a share of a line and
/// at least this many pixels: wider than a stroke, so the median is the surrounding fill.
const NEIGHBOURHOOD_SHARE: f64 = 0.25;
const MIN_NEIGHBOURHOOD_PX: u32 = 4;
/// Samples per side of the neighbourhood grid.
const GRID: u32 = 9;
/// How much wider the mask grows for the one retry, as a share of a line.
const RETRY_GROWTH_SHARE: f64 = 0.1;
/// Half the least thickness of a remaining blob, as a share of the stroke thickness.
const THICKNESS_SHARE: f64 = 0.125;

/// Share of `mask`'s set pixels that still look like the lettering `style` describes: within
/// [`INK_DELTA_E`] of its fill colour, more than [`STANDS_OUT_DELTA_E`] from the median of the
/// fill around them, and in a blob at least a quarter of a stroke thick, so thin texture lines
/// the fill continues (joints, cracks) do not count.
pub fn residue_share(filled: &RgbImage, mask: &GrayImage, style: &LetteringStyle) -> f64 {
    let (w, h) = filled.dimensions();
    let radius = MIN_NEIGHBOURHOOD_PX
        .max(
            (NEIGHBOURHOOD_SHARE * style.line_height_px)
                .round()
                .max(0.0) as u32,
        )
        .min(w.max(h));
    let step = (2 * radius / (GRID - 1)).max(1);
    let mut masked = 0usize;
    let mut remaining = GrayImage::new(w, h);
    for (x, y, m) in mask.enumerate_pixels() {
        if m.0[0] == 0 {
            continue;
        }
        masked += 1;
        let pixel = filled.get_pixel(x, y).0;
        if delta_e(pixel, style.fill_rgb) > INK_DELTA_E {
            continue;
        }
        let median = neighbourhood_median(filled, (x, y), radius, step);
        if delta_e(pixel, median) > STANDS_OUT_DELTA_E {
            remaining.put_pixel(x, y, Luma([255]));
        }
    }
    let thickness = (THICKNESS_SHARE * style.stroke_px).round().clamp(1.0, 64.0) as u8;
    let blobs = open(&remaining, Norm::LInf, thickness);
    let count = blobs.pixels().filter(|p| p.0[0] > 0).count();
    count as f64 / masked.max(1) as f64
}

/// The mask grown for the retry: by a tenth of a line, at least one pixel.
pub(super) fn widened(mask: &GrayImage, line_height: f64) -> GrayImage {
    let growth = (RETRY_GROWTH_SHARE * line_height)
        .round()
        .clamp(1.0, f64::from(u8::MAX)) as u8;
    let grown = dilate(mask, Norm::L2, growth);
    GrayImage::from_fn(mask.width(), mask.height(), |x, y| {
        Luma([if grown.get_pixel(x, y).0[0] > 0 {
            255
        } else {
            0
        }])
    })
}

/// Per-channel median of the plate on a grid of up to [`GRID`] × [`GRID`] points `step` apart
/// around `(x, y)`, clipped to the plate.
fn neighbourhood_median(image: &RgbImage, (x, y): (u32, u32), radius: u32, step: u32) -> [u8; 3] {
    let (w, h) = image.dimensions();
    let axis = |centre: u32, len: u32| {
        let first = centre.saturating_sub(radius);
        let last = (centre + radius).min(len - 1);
        (first..=last).step_by(step as usize)
    };
    let mut channels: [Vec<u8>; 3] = Default::default();
    for sy in axis(y, h) {
        for sx in axis(x, w) {
            let p = image.get_pixel(sx, sy).0;
            for (channel, &value) in channels.iter_mut().zip(&p) {
                channel.push(value);
            }
        }
    }
    channels.map(|mut values| {
        values.sort_unstable();
        values[values.len() / 2]
    })
}

#[cfg(test)]
#[path = "tests/residue.rs"]
mod tests;
