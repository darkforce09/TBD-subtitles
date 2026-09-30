//! Anti-aliased rasterizing of lettering onto a supersampled rectified canvas.
//!
//! **Role:** turn glyph outlines and colours into premultiplied RGBA pixels of the rectified
//! writing area.
//! **Position:** after layout and colours, before the warp onto each plate.
//! **Signals and state:** one canvas per occurrence, at most `MAX_SIDE` pixels on a side.
//! **Invariants:** the outline sits under the fill; a soft outline blurs only the outline's
//! coverage; pixels are premultiplied.

use tiny_skia::{FillRule, LineCap, LineJoin, Mask, Path, Stroke, Transform};

use super::colours::Colours;
use super::layout::Area;
use crate::onscreen_text::TextResult;

/// Canvas pixels per source pixel when the area is small enough.
pub(crate) const SUPERSAMPLE: f64 = 2.0;
/// The longest canvas side in pixels.
const MAX_SIDE: f64 = 4096.0;

/// Premultiplied RGBA pixels of the rectified writing area.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Canvas {
    pub width: u32,
    pub height: u32,
    /// Canvas pixels per area pixel.
    pub scale: f64,
    pub pixels: Vec<[u8; 4]>,
}

impl Canvas {
    /// An empty canvas for `area` at the supersampling scale, bounded to `MAX_SIDE`.
    pub(crate) fn blank(area: Area) -> TextResult<Self> {
        let longest = area.width.max(area.height);
        if !(longest.is_finite() && longest > 0.0) {
            return Err("The writing area has no size".into());
        }
        let scale = SUPERSAMPLE.min(MAX_SIDE / longest);
        let width = (area.width * scale).ceil().max(1.0) as u32;
        let height = (area.height * scale).ceil().max(1.0) as u32;
        Ok(Self {
            width,
            height,
            scale,
            pixels: vec![[0; 4]; width as usize * height as usize],
        })
    }
}

/// Paint `glyphs` (in canvas pixels) onto `canvas` with `colours`; outline widths are in area
/// pixels.
pub(crate) fn paint(canvas: &mut Canvas, glyphs: &Path, colours: &Colours) -> TextResult<()> {
    let fill = coverage(canvas, glyphs)?;
    let outline = match colours.outline {
        Some(outline) => {
            let width = outline.width * canvas.scale;
            let stroke = Stroke {
                width: (2.0 * width) as f32,
                line_join: LineJoin::Round,
                line_cap: LineCap::Round,
                ..Stroke::default()
            };
            let mut mask = coverage(canvas, glyphs)?;
            if let Some(stroked) = glyphs.stroke(&stroke, 1.0) {
                mask.fill_path(&stroked, FillRule::Winding, true, Transform::identity());
            }
            let mut values = mask.data().to_vec();
            if outline.soft {
                let radius = (width / 2.0).round().max(1.0) as usize;
                box_blur(
                    &mut values,
                    canvas.width as usize,
                    canvas.height as usize,
                    radius,
                );
            }
            Some((outline.rgb, values))
        }
        None => None,
    };
    let fill_rgb = colours.fill.map(f32::from);
    for (index, pixel) in canvas.pixels.iter_mut().enumerate() {
        let a_fill = f32::from(fill.data()[index]) / 255.0;
        let (outline_rgb, a_outline) = match &outline {
            Some((rgb, values)) => (rgb.map(f32::from), f32::from(values[index]) / 255.0),
            None => ([0.0; 3], 0.0),
        };
        let under = a_outline * (1.0 - a_fill);
        let alpha = a_fill + under;
        let channel = |c: usize| (fill_rgb[c] * a_fill + outline_rgb[c] * under).round() as u8;
        *pixel = [
            channel(0),
            channel(1),
            channel(2),
            (alpha * 255.0).round() as u8,
        ];
    }
    Ok(())
}

fn coverage(canvas: &Canvas, path: &Path) -> TextResult<Mask> {
    let mut mask = Mask::new(canvas.width, canvas.height).ok_or("The lettering canvas is empty")?;
    mask.fill_path(path, FillRule::Winding, true, Transform::identity());
    Ok(mask)
}

/// Separable box blur of an 8-bit coverage buffer; edges repeat.
pub(crate) fn box_blur(values: &mut [u8], width: usize, height: usize, radius: usize) {
    if radius == 0 || width == 0 || height == 0 {
        return;
    }
    let mut row = vec![0u32; width.max(height)];
    for y in 0..height {
        for x in 0..width {
            row[x] = u32::from(values[y * width + x]);
        }
        blur_line(&row[..width], radius, |x, v| values[y * width + x] = v);
    }
    for x in 0..width {
        for y in 0..height {
            row[y] = u32::from(values[y * width + x]);
        }
        blur_line(&row[..height], radius, |y, v| values[y * width + x] = v);
    }
}

fn blur_line(line: &[u32], radius: usize, mut write: impl FnMut(usize, u8)) {
    let last = line.len() - 1;
    let taps = (2 * radius + 1) as u32;
    let at = |i: isize| line[i.clamp(0, last as isize) as usize];
    let mut sum: u32 = (-(radius as isize)..=radius as isize).map(at).sum();
    for index in 0..line.len() {
        write(index, ((sum + taps / 2) / taps) as u8);
        let i = index as isize;
        sum = sum + at(i + radius as isize + 1) - at(i - radius as isize);
    }
}

#[cfg(test)]
#[path = "tests/render.rs"]
mod tests;
