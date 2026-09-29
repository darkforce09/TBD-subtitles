//! Rectified crops, text surfaces and keyframe stills.
//!
//! **Role:** rectify a quad's lettering plane, measure a plain surface behind writing, and confirm
//! every occurrence on the full-resolution still of its keyframe.
//! **Position:** the scan's last phase, and the crop helper of the validation harness.
//! **Signals and state:** at most four full-resolution stills at a time; crop and keyframe PNGs
//! under the job's `visual/` folder.
//! **Invariants:** one still and one detector pass per keyframe frame; every occurrence gets one
//! crop and one keyframe image; an unconfirmed region keeps its screening geometry and a warning;
//! one surface measurement covers all of an occurrence's frames.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::{RgbImage, imageops};
use imageproc::geometric_transformations::{Interpolation, Projection, warp_into};
use inference::ocr::TextDetection;
use job_model::onscreen::{Point, Quad, TextDocument, TextKeyframe, TextOccurrence};

use super::regions::{SAME_REGION, overlap};
use super::source::FrameSource;
use crate::onscreen_text::{TextResult, geometry};

/// The warning on an occurrence whose keyframe still shows no matching full-resolution region.
pub(super) const UNCONFIRMED: &str = "The keyframe detector did not confirm this region; its geometry comes from the screening copy.";
/// Stills requested from the frame source at once.
const STILL_CHUNK: usize = 4;
/// The widest saved keyframe still, in pixels.
const KEYFRAME_WIDTH: u32 = 1280;

/// Confirms every occurrence on the still of its keyframe, `(frame position, frame index)` per
/// occurrence, fetching the distinct stills four at a time in the order first needed.
pub(super) fn confirm_keyframes(
    document: &mut TextDocument,
    keyframes: &[Option<(usize, u64)>],
    source: &mut dyn FrameSource,
    detector: &mut dyn TextDetection,
    root: &Path,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> TextResult<()> {
    std::fs::create_dir_all(root.join("visual/crops"))?;
    std::fs::create_dir_all(root.join("visual/keyframes"))?;
    let mut order = Vec::new();
    let mut users: HashMap<u64, Vec<usize>> = HashMap::new();
    for (occurrence, keyframe) in keyframes.iter().enumerate() {
        if let Some((_, index)) = *keyframe {
            let sharing = users.entry(index).or_default();
            if sharing.is_empty() {
                order.push(index);
            }
            sharing.push(occurrence);
        }
    }
    let decoded = usize::try_from(document.decoded_frames).unwrap_or(usize::MAX);
    let total = decoded.saturating_add(order.len());
    let mut done = decoded;
    for chunk in order.chunks(STILL_CHUNK) {
        let stills = source.stills(chunk)?;
        if stills.len() != chunk.len() {
            return Err("The frame source returned a different number of stills".into());
        }
        for (&index, still) in chunk.iter().zip(&stills) {
            if still.dimensions() != (document.width, document.height) {
                return Err("A keyframe still does not match the source video dimensions".into());
            }
            let found = detector.detect(still)?;
            let image = PathBuf::from(format!("visual/keyframes/frame-{index:08}.png"));
            save_keyframe(still, &root.join(&image))?;
            for &occurrence in users.get(&index).into_iter().flatten() {
                if let Some((position, _)) = keyframes[occurrence] {
                    let item = &mut document.occurrences[occurrence];
                    confirm(item, position, still, &found, &image, root)?;
                }
            }
            done += 1;
            progress(done, total);
        }
    }
    Ok(())
}

/// Replaces the keyframe frame's quad by the full-resolution region overlapping it most, then
/// measures the surface, saves the rectified crop and records the keyframe.
pub(super) fn confirm(
    item: &mut TextOccurrence,
    position: usize,
    still: &RgbImage,
    found: &[(Quad, f64)],
    image: &Path,
    root: &Path,
) -> TextResult<()> {
    let Some(frame) = item.frames.get_mut(position) else {
        return Err("A keyframe lies outside its occurrence's frames".into());
    };
    let best = found
        .iter()
        .map(|&(quad, _)| (overlap(frame.quad, quad), quad))
        .filter(|&(share, _)| share > SAME_REGION)
        .max_by(|a, b| a.0.total_cmp(&b.0));
    match best {
        Some((_, quad)) => frame.quad = quad,
        None => {
            if !item.warnings.iter().any(|warning| warning == UNCONFIRMED) {
                item.warnings.push(UNCONFIRMED.to_string());
            }
        }
    }
    let (quad, time_s) = (frame.quad, frame.time_s);
    let surface = simple_surface(&axis_crop(still, quad));
    for frame in &mut item.frames {
        frame.surface_rgb = surface;
    }
    let crop_path = PathBuf::from(format!("visual/crops/{}.png", item.id));
    crop(still, quad).save(root.join(&crop_path))?;
    item.crops = vec![crop_path];
    item.keyframe = Some(TextKeyframe {
        time_s,
        image: image.to_path_buf(),
    });
    Ok(())
}

/// Saves a still at most 1280 pixels wide, keeping its aspect.
pub(super) fn save_keyframe(still: &RgbImage, path: &Path) -> TextResult<()> {
    if still.width() <= KEYFRAME_WIDTH {
        still.save(path)?;
        return Ok(());
    }
    let height = (f64::from(still.height()) * f64::from(KEYFRAME_WIDTH) / f64::from(still.width()))
        .round()
        .max(1.0) as u32;
    imageops::resize(
        still,
        KEYFRAME_WIDTH,
        height,
        imageops::FilterType::Triangle,
    )
    .save(path)?;
    Ok(())
}

/// The quad's lettering plane rectified to an upright image; an axis-aligned or degenerate quad
/// is cropped directly.
pub fn crop(image: &RgbImage, quad: Quad) -> RgbImage {
    let p = quad.0;
    let axis_aligned = (p[0].y - p[1].y).abs() < 0.1
        && (p[2].y - p[3].y).abs() < 0.1
        && (p[0].x - p[3].x).abs() < 0.1
        && (p[1].x - p[2].x).abs() < 0.1;
    if axis_aligned {
        return axis_crop(image, quad);
    }
    let width = geometry::distance(p[0], p[1])
        .max(geometry::distance(p[3], p[2]))
        .ceil() as u32;
    let height = geometry::distance(p[0], p[3])
        .max(geometry::distance(p[1], p[2]))
        .ceil() as u32;
    if width < 2 || height < 2 || u64::from(width) * u64::from(height) > 16_777_216 {
        return axis_crop(image, quad);
    }
    let target = Quad([
        Point { x: 0.0, y: 0.0 },
        Point {
            x: f64::from(width - 1),
            y: 0.0,
        },
        Point {
            x: f64::from(width - 1),
            y: f64::from(height - 1),
        },
        Point {
            x: 0.0,
            y: f64::from(height - 1),
        },
    ]);
    let Some(transform) = geometry::quad_to_quad(quad, target) else {
        return axis_crop(image, quad);
    };
    let Some(projection) =
        Projection::from_matrix(std::array::from_fn(|i| transform[(i / 3, i % 3)] as f32))
    else {
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
pub(super) fn axis_crop(image: &RgbImage, quad: Quad) -> RgbImage {
    let (left, top, right, bottom) = quad.bounds();
    let x = (left.floor().max(0.0) as u32).min(image.width().saturating_sub(1));
    let y = (top.floor().max(0.0) as u32).min(image.height().saturating_sub(1));
    let w = ((right.ceil() as u32).min(image.width()).saturating_sub(x)).max(1);
    let h = ((bottom.ceil() as u32).min(image.height()).saturating_sub(y)).max(1);
    imageops::crop_imm(image, x, y, w, h).to_image()
}

/// A mask requires a nearly constant border and a dominant constant interior background.
pub(super) fn simple_surface(image: &RgbImage) -> Option<[u8; 3]> {
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

#[cfg(test)]
#[path = "tests/crops.rs"]
mod tests;
