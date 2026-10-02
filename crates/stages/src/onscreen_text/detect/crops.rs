//! Rectified crops, grey region pictures, text surfaces and the confirmation of one occurrence.
//!
//! **Role:** rectify a quad's lettering plane in colour or in grey from the luma plane, measure a
//! plain surface behind writing, and confirm an occurrence on the full-resolution still of its
//! keyframe.
//! **Position:** used by the scan for region signatures, by confirmation for crops, and by the
//! validation harness and the read-back check for `crop`.
//! **Signals and state:** none; the caller writes the crops this module returns.
//! **Invariants:** a grey picture reads the full-range luma plane with no colour conversion and
//! covers the same pixels the colour crop would; one surface measurement covers all of an
//! occurrence's frames; an unconfirmed occurrence is left untouched.

use std::path::{Path, PathBuf};

use image::{GrayImage, Luma, RgbImage, imageops};
use imageproc::geometric_transformations::{Interpolation, Projection, warp_into};
use job_model::onscreen::{Point, Quad, TextKeyframe, TextOccurrence};
use media_io::yuv::{Coefficients, Rect, Yuv420, crop_grey};

use super::regions::{SAME_REGION, overlap, signature};
use crate::onscreen_text::{TextResult, geometry};

/// Pixels around a quad's bounds that a grey rectification also reads, for its bilinear taps.
const WARP_MARGIN: usize = 2;
/// The largest rectified crop, in pixels; a larger plane is cropped by its bounds instead.
const MAX_RECTIFIED: u64 = 16_777_216;

/// Replaces the keyframe frame's quad by the full-resolution region overlapping it most, then
/// measures the surface and records the keyframe; the rectified crop to write and its path come
/// back. Without an overlapping region the occurrence is unconfirmed and left untouched: `None`.
pub(crate) fn confirm(
    item: &mut TextOccurrence,
    position: usize,
    still: &RgbImage,
    found: &[(Quad, f64)],
    image: &Path,
) -> TextResult<Option<(PathBuf, RgbImage)>> {
    let Some(frame) = item.frames.get_mut(position) else {
        return Err("A keyframe lies outside its occurrence's frames".into());
    };
    let best = found
        .iter()
        .map(|&(quad, _)| (overlap(frame.quad, quad), quad))
        .filter(|&(share, _)| share > SAME_REGION)
        .max_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, quad)) = best else {
        return Ok(None);
    };
    frame.quad = quad;
    let time_s = frame.time_s;
    let surface = simple_surface(&axis_crop(still, quad));
    for frame in &mut item.frames {
        frame.surface_rgb = surface;
    }
    let crop_path = PathBuf::from(format!("visual/crops/{}.png", item.id));
    item.crops = vec![crop_path.clone()];
    item.keyframe = Some(TextKeyframe {
        time_s,
        image: image.to_path_buf(),
    });
    Ok(Some((crop_path, crop(still, quad))))
}

/// The signature of `picture` at a region's anchor box, read from its luma plane, so pictures
/// of one region are always compared over the same pixels whatever box the detector draws on a
/// given frame.
pub(crate) fn picture_at(
    picture: &Yuv420<'_>,
    colour: &Coefficients,
    anchor_box: Quad,
) -> GrayImage {
    signature(&grey_crop(picture, colour, anchor_box))
}

/// The quad's lettering plane rectified to an upright grey image from the luma plane; an
/// axis-aligned or degenerate quad is cropped directly.
pub(crate) fn grey_crop(picture: &Yuv420<'_>, colour: &Coefficients, quad: Quad) -> GrayImage {
    let Some((width, height, _)) = rectification(quad) else {
        return grey_rect(
            picture,
            colour,
            bounds_rect(quad, picture.width, picture.height, 0),
        );
    };
    // Only the quad's bounds, with a margin for the interpolation, are read from the plane.
    let area = bounds_rect(quad, picture.width, picture.height, WARP_MARGIN);
    let source = grey_rect(picture, colour, area);
    let shifted = Quad(quad.0.map(|point| Point {
        x: point.x - area.x as f64,
        y: point.y - area.y as f64,
    }));
    let Some((_, _, projection)) = rectification(shifted) else {
        return grey_rect(
            picture,
            colour,
            bounds_rect(quad, picture.width, picture.height, 0),
        );
    };
    let mut output = GrayImage::new(width, height);
    warp_into(
        &source,
        &projection,
        Interpolation::Bilinear,
        Luma([255]),
        &mut output,
    );
    output
}

/// The full-range grey picture of `rect`, which lies inside the picture.
fn grey_rect(picture: &Yuv420<'_>, colour: &Coefficients, rect: Rect) -> GrayImage {
    let mut grey = Vec::new();
    if !crop_grey(picture, colour, rect, &mut grey) {
        return GrayImage::new(1, 1);
    }
    GrayImage::from_raw(rect.width as u32, rect.height as u32, grey)
        .unwrap_or_else(|| GrayImage::new(1, 1))
}

/// The quad's bounding box grown by `margin`, clamped to a `width` × `height` picture and at
/// least one pixel, as `axis_crop` takes it.
fn bounds_rect(quad: Quad, width: usize, height: usize, margin: usize) -> Rect {
    let (left, top, right, bottom) = quad.bounds();
    let margin = margin as f64;
    let x = ((left - margin).floor().max(0.0) as usize).min(width.saturating_sub(1));
    let y = ((top - margin).floor().max(0.0) as usize).min(height.saturating_sub(1));
    let w = (((right + margin).ceil().max(0.0) as usize).min(width)).saturating_sub(x);
    let h = (((bottom + margin).ceil().max(0.0) as usize).min(height)).saturating_sub(y);
    Rect {
        x,
        y,
        width: w.max(1),
        height: h.max(1),
    }
}

/// The rectified size of a quad's lettering plane and the projection onto it; `None` when the
/// quad is axis-aligned, degenerate or too large, and is cropped by its bounds instead.
fn rectification(quad: Quad) -> Option<(u32, u32, Projection)> {
    let p = quad.0;
    let axis_aligned = (p[0].y - p[1].y).abs() < 1.5
        && (p[2].y - p[3].y).abs() < 1.5
        && (p[0].x - p[3].x).abs() < 1.5
        && (p[1].x - p[2].x).abs() < 1.5;
    if axis_aligned {
        return None;
    }
    let width = geometry::distance(p[0], p[1])
        .max(geometry::distance(p[3], p[2]))
        .ceil() as u32;
    let height = geometry::distance(p[0], p[3])
        .max(geometry::distance(p[1], p[2]))
        .ceil() as u32;
    if width < 2 || height < 2 || u64::from(width) * u64::from(height) > MAX_RECTIFIED {
        return None;
    }
    let (right, bottom) = (f64::from(width - 1), f64::from(height - 1));
    let target = Quad([
        Point { x: 0.0, y: 0.0 },
        Point { x: right, y: 0.0 },
        Point {
            x: right,
            y: bottom,
        },
        Point { x: 0.0, y: bottom },
    ]);
    let transform = geometry::quad_to_quad(quad, target)?;
    let projection =
        Projection::from_matrix(std::array::from_fn(|i| transform[(i / 3, i % 3)] as f32))?;
    Some((width, height, projection))
}

/// The quad's lettering plane rectified to an upright image; an axis-aligned or degenerate quad
/// is cropped directly.
pub fn crop(image: &RgbImage, quad: Quad) -> RgbImage {
    let Some((width, height, projection)) = rectification(quad) else {
        return axis_crop(image, quad);
    };
    let mut output = RgbImage::new(width, height);
    warp_into(
        image,
        &projection,
        Interpolation::Bilinear,
        image::Rgb([255; 3]),
        &mut output,
    );
    output
}

/// The quad's bounding box, clamped to the image and at least one pixel.
pub(crate) fn axis_crop(image: &RgbImage, quad: Quad) -> RgbImage {
    let rect = bounds_rect(quad, image.width() as usize, image.height() as usize, 0);
    imageops::crop_imm(
        image,
        rect.x as u32,
        rect.y as u32,
        rect.width as u32,
        rect.height as u32,
    )
    .to_image()
}

/// A mask requires a nearly constant border and a dominant constant interior background.
pub(crate) fn simple_surface(image: &RgbImage) -> Option<[u8; 3]> {
    let color = image.get_pixel(0, 0).0;
    let similar =
        |pixel: &image::Rgb<u8>| pixel.0.iter().zip(color).all(|(a, b)| a.abs_diff(b) <= 5);
    let border = (0..image.width())
        .all(|x| similar(image.get_pixel(x, 0)) && similar(image.get_pixel(x, image.height() - 1)))
        && (0..image.height()).all(|y| {
            similar(image.get_pixel(0, y)) && similar(image.get_pixel(image.width() - 1, y))
        });
    if !border {
        return None;
    }
    let share = image.pixels().filter(|pixel| similar(pixel)).count() as f64
        / f64::from(image.width() * image.height());
    (share > 0.8).then_some(color)
}

/// Whether `quad` on `picture` has sufficient contrast to be readable text. Flat surfaces
/// (stone walls, sky, smooth skin) lack the variance and dynamic range of real lettering.
pub(crate) fn has_text_contrast(picture: &Yuv420<'_>, colour: &Coefficients, quad: Quad) -> bool {
    let rect = bounds_rect(quad, picture.width, picture.height, 0);
    let mut grey = Vec::with_capacity(rect.width * rect.height);
    if !crop_grey(picture, colour, rect, &mut grey) || grey.is_empty() {
        return false;
    }
    let (mut min, mut max, mut sum) = (u8::MAX, 0u8, 0u64);
    for &sample in &grey {
        min = min.min(sample);
        max = max.max(sample);
        sum += u64::from(sample);
    }
    if max.saturating_sub(min) < 35 {
        return false;
    }
    let mean = sum / grey.len() as u64;
    let variance: u64 = grey
        .iter()
        .map(|&s| {
            let diff = s as i64 - mean as i64;
            (diff * diff) as u64
        })
        .sum::<u64>()
        / grey.len() as u64;
    variance >= 64 // std dev >= 8.0
}

#[cfg(test)]
#[path = "tests/crops.rs"]
mod tests;
