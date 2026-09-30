//! Perspective warp of the rectified lettering canvas onto a plate.
//!
//! **Role:** place the rendered lettering on the writing's quad in plate pixels.
//! **Position:** after rendering, before the patch is composited.
//! **Signals and state:** one canvas in; premultiplied RGBA in 0..1 per plate pixel out.
//! **Invariants:** every plate pixel samples the canvas at its centre through the inverse
//! homography with bilinear filtering; pixels outside the canvas stay transparent.

use job_model::onscreen::{Point, Quad};

use super::layout::Area;
use super::render::Canvas;
use crate::onscreen_text::geometry;

/// The canvas of `area` mapped onto `quad` in a `width` × `height` plate; `None` when the quad
/// admits no invertible mapping.
pub(crate) fn warp(
    canvas: &Canvas,
    area: Area,
    quad: Quad,
    width: u32,
    height: u32,
) -> Option<Vec<[f32; 4]>> {
    let source = Quad([
        Point { x: 0.0, y: 0.0 },
        Point {
            x: area.width,
            y: 0.0,
        },
        Point {
            x: area.width,
            y: area.height,
        },
        Point {
            x: 0.0,
            y: area.height,
        },
    ]);
    let inverse = geometry::quad_to_quad(source, quad)?.try_inverse()?;
    let mut output = vec![[0.0f32; 4]; width as usize * height as usize];
    let (left, top, right, bottom) = quad.bounds();
    let clamp = |value: f64, limit: u32| value.clamp(0.0, f64::from(limit)) as u32;
    let (x0, x1) = (
        clamp(left.floor() - 1.0, width),
        clamp(right.ceil() + 1.0, width),
    );
    let (y0, y1) = (
        clamp(top.floor() - 1.0, height),
        clamp(bottom.ceil() + 1.0, height),
    );
    for y in y0..y1 {
        for x in x0..x1 {
            let centre = Point {
                x: f64::from(x) + 0.5,
                y: f64::from(y) + 0.5,
            };
            let Some(local) = geometry::project(&inverse, centre) else {
                continue;
            };
            output[y as usize * width as usize + x as usize] = sample(
                canvas,
                local.x * canvas.scale - 0.5,
                local.y * canvas.scale - 0.5,
            );
        }
    }
    Some(output)
}

/// Bilinear sample of premultiplied canvas pixels at a pixel-centre coordinate, in 0..1.
fn sample(canvas: &Canvas, x: f64, y: f64) -> [f32; 4] {
    let (w, h) = (i64::from(canvas.width), i64::from(canvas.height));
    if !(x.is_finite() && y.is_finite()) || x <= -1.0 || y <= -1.0 {
        return [0.0; 4];
    }
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i64, fy as i64);
    if ix >= w || iy >= h {
        return [0.0; 4];
    }
    let (tx, ty) = ((x - fx) as f32, (y - fy) as f32);
    let texel = |cx: i64, cy: i64| -> [f32; 4] {
        if cx < 0 || cy < 0 || cx >= w || cy >= h {
            return [0.0; 4];
        }
        canvas.pixels[(cy * w + cx) as usize].map(|v| f32::from(v) / 255.0)
    };
    let (a, b, c, d) = (
        texel(ix, iy),
        texel(ix + 1, iy),
        texel(ix, iy + 1),
        texel(ix + 1, iy + 1),
    );
    let mut value = [0.0f32; 4];
    for channel in 0..4 {
        let top = a[channel] + (b[channel] - a[channel]) * tx;
        let bottom = c[channel] + (d[channel] - c[channel]) * tx;
        value[channel] = top + (bottom - top) * ty;
    }
    value
}

#[cfg(test)]
#[path = "tests/warp.rs"]
mod tests;
