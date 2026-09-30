//! Lettering colours: the measured fill and outline, plus a legibility outline when needed.
//!
//! **Role:** decide the fill and outline of one occurrence's English lettering.
//! **Position:** colour rules under `compose`, before rendering.
//! **Signals and state:** the measured style and the mean background colour in; `Colours` out.
//! **Invariants:** a measured outline is always kept; without one, lettering whose fill
//! contrasts below 3:1 (WCAG) with its background gets a 2-pixel black or white outline.

use image::RgbImage;
use job_model::onscreen::{LetteringStyle, Point, Quad};

use crate::onscreen_text::geometry;

/// The least WCAG contrast between fill and background that needs no added outline.
const MIN_CONTRAST: f64 = 3.0;
/// Width in source pixels of an outline added for legibility.
const ADDED_OUTLINE_PX: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Outline {
    pub rgb: [u8; 3],
    /// Width outside the glyph edge in source pixels.
    pub width: f64,
    /// The outline fades out instead of ending in a hard edge.
    pub soft: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Colours {
    pub fill: [u8; 3],
    pub outline: Option<Outline>,
}

/// The colours for `style` drawn over a background whose mean colour is `background`.
pub(crate) fn colours(style: &LetteringStyle, background: [f64; 3]) -> Colours {
    let outline = match style.outline_rgb {
        Some(rgb) => Some(Outline {
            rgb,
            width: style.outline_px.max(1.0),
            soft: style.soft_outline,
        }),
        None if contrast(luminance_u8(style.fill_rgb), luminance(background)) < MIN_CONTRAST => {
            let fill = luminance_u8(style.fill_rgb);
            let rgb = if contrast(fill, 0.0) >= contrast(fill, 1.0) {
                [0, 0, 0]
            } else {
                [255, 255, 255]
            };
            Some(Outline {
                rgb,
                width: ADDED_OUTLINE_PX,
                soft: false,
            })
        }
        None => None,
    };
    Colours {
        fill: style.fill_rgb,
        outline,
    }
}

/// Mean colour of the plate pixels whose centres lie inside `quad`; the whole plate's mean when
/// none does.
pub(crate) fn mean_under(plate: &RgbImage, quad: Quad) -> [f64; 3] {
    let mut inside = [0.0f64; 3];
    let mut inside_count = 0u64;
    let mut all = [0.0f64; 3];
    for (x, y, pixel) in plate.enumerate_pixels() {
        let values = pixel.0.map(f64::from);
        for channel in 0..3 {
            all[channel] += values[channel];
        }
        let centre = Point {
            x: f64::from(x) + 0.5,
            y: f64::from(y) + 0.5,
        };
        if geometry::contains(quad, centre) {
            for channel in 0..3 {
                inside[channel] += values[channel];
            }
            inside_count += 1;
        }
    }
    let (sum, count) = if inside_count > 0 {
        (inside, inside_count)
    } else {
        (all, u64::from(plate.width()) * u64::from(plate.height()))
    };
    if count == 0 {
        return [0.0; 3];
    }
    sum.map(|value| value / count as f64)
}

/// WCAG contrast ratio of two relative luminances.
pub(crate) fn contrast(a: f64, b: f64) -> f64 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// WCAG relative luminance of an sRGB colour with 0..255 channels.
pub(crate) fn luminance(rgb: [f64; 3]) -> f64 {
    let linear = rgb.map(|value| {
        let c = (value / 255.0).clamp(0.0, 1.0);
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
}

fn luminance_u8(rgb: [u8; 3]) -> f64 {
    luminance(rgb.map(f64::from))
}

#[cfg(test)]
#[path = "tests/colours.rs"]
mod tests;
