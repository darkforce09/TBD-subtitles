//! The application icon, drawn in code: a rounded dark tile with two white subtitle bars.
//!
//! **Role:** paint the icon at any square size with 4×4 supersampled edges and encode it as an
//! RGBA PNG with the `png` crate.
//!
//! **Position:** part of `app_dir`, which writes the PNG at the AppDir root and under
//! `usr/share/icons`.
//!
//! **Signals and state:** pure; returns bytes.
//!
//! **Invariants:** the corners outside the tile are fully transparent; the same size always gives
//! the same bytes.

use anyhow::{Context, Result};

/// The tile's colour: the dark window background of the app.
const TILE: [f32; 3] = [34.0, 37.0, 45.0];
/// The subtitle bars' colour.
const BAR: [f32; 3] = [245.0, 245.0, 245.0];
/// Samples per pixel along each axis.
const SUPERSAMPLE: u32 = 4;

/// A rounded rectangle in unit coordinates (0..1 across the icon).
struct Rounded {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
}

impl Rounded {
    fn contains(&self, x: f32, y: f32) -> bool {
        if x < self.left || x > self.right || y < self.top || y > self.bottom {
            return false;
        }
        let cx = x.clamp(self.left + self.radius, self.right - self.radius);
        let cy = y.clamp(self.top + self.radius, self.bottom - self.radius);
        (x - cx).powi(2) + (y - cy).powi(2) <= self.radius.powi(2)
    }
}

/// The tile and the two bars, lower one shorter, as on a screen showing a two-line subtitle.
const SHAPES: [(Rounded, [f32; 3]); 3] = [
    (
        Rounded {
            left: 0.06,
            top: 0.06,
            right: 0.94,
            bottom: 0.94,
            radius: 0.19,
        },
        TILE,
    ),
    (
        Rounded {
            left: 0.20,
            top: 0.56,
            right: 0.80,
            bottom: 0.645,
            radius: 0.04,
        },
        BAR,
    ),
    (
        Rounded {
            left: 0.29,
            top: 0.70,
            right: 0.71,
            bottom: 0.785,
            radius: 0.04,
        },
        BAR,
    ),
];

/// The icon as a `size`×`size` RGBA PNG.
pub(crate) fn icon_png(size: u32) -> Result<Vec<u8>> {
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    let step = 1.0 / (size * SUPERSAMPLE) as f32;
    for py in 0..size {
        for px in 0..size {
            let mut sum = [0.0f32; 4];
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let x = (px * SUPERSAMPLE + sx) as f32 * step + step / 2.0;
                    let y = (py * SUPERSAMPLE + sy) as f32 * step + step / 2.0;
                    if let Some(colour) = colour_at(x, y) {
                        sum[0] += colour[0];
                        sum[1] += colour[1];
                        sum[2] += colour[2];
                        sum[3] += 1.0;
                    }
                }
            }
            let samples = (SUPERSAMPLE * SUPERSAMPLE) as f32;
            let alpha = sum[3] / samples;
            let channel = |c: f32| {
                if sum[3] > 0.0 {
                    (c / sum[3]).round() as u8
                } else {
                    0
                }
            };
            pixels.extend_from_slice(&[
                channel(sum[0]),
                channel(sum[1]),
                channel(sum[2]),
                (alpha * 255.0).round() as u8,
            ]);
        }
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .context("writing the icon's PNG header")?;
    writer
        .write_image_data(&pixels)
        .context("writing the icon's pixels")?;
    writer.finish().context("finishing the icon PNG")?;
    Ok(out)
}

/// The colour of the topmost shape at a point, or none outside the tile.
fn colour_at(x: f32, y: f32) -> Option<[f32; 3]> {
    SHAPES
        .iter()
        .rev()
        .find(|(shape, _)| shape.contains(x, y))
        .map(|(_, colour)| *colour)
}
