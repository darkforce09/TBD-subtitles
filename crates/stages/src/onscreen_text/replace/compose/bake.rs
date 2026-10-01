//! Patch files for one occurrence: render once, warp onto every plate, write, or fall back.
//!
//! **Role:** turn one laid-out occurrence into its patch and preview files and record them.
//! **Position:** the file side of `compose`, after layout.
//! **Signals and state:** reads the plate, mask and source PNGs one plate at a time; writes
//! `visual/patches/<id>/<n>.png` for plate `n` at its own shift, `<n>-<k>.png` for each further
//! shift its frames take, and `preview.png`.
//! **Invariants:** an occurrence's patch folder is rebuilt from nothing on every run; a fallback
//! removes it and clears every patch path; missing or mis-sized input images are errors.

use std::path::{Path, PathBuf};

use image::{DynamicImage, GrayImage, RgbImage};
use job_model::onscreen::{PixelRect, ReplaceStatus, ReplacedText, ShiftedPatch};

use super::Prepared;
use super::colours;
use super::containers;
use super::font::FontMetrics;
use super::layout::{self, Layout};
use super::patch;
use super::render::{self, Canvas};
use super::warp;
use crate::localize::motion::Motion;
use crate::onscreen_text::TextResult;

/// The patch folder, relative to the job directory.
const PATCHES: &str = "visual/patches";
/// Why lettering whose quad does not map onto a plate falls back.
const UNMAPPED: &str = "The writing's position cannot be mapped onto its background";

/// Letter `item` and write its patches; `Ok(Err(reason))` when it must fall back instead.
pub(super) fn bake(
    root: &Path,
    item: &mut ReplacedText,
    ready: &Prepared,
    metrics: &FontMetrics<'_>,
    layout: &Layout,
    motion: &Motion,
) -> TextResult<Result<(), String>> {
    let folder = folder(&item.id);
    reset(&root.join(&folder))?;
    std::fs::create_dir_all(root.join(&folder))
        .map_err(|e| format!("Cannot create {}: {e}", root.join(&folder).display()))?;
    let key = containers::plate_at(&item.plates, ready.keyframe).unwrap_or(0);
    let key_plate = &item.plates[key];
    let key_image = plate_image(root, key_plate.plate.as_deref(), key_plate.rect, &item.id)?;
    let colours = colours::colours(
        &ready.style,
        colours::mean_under(&key_image, containers::plate_quad(ready.quad, key_plate)),
    );
    drop(key_image);
    let mut canvas = Canvas::blank(ready.area)?;
    let lines = layout::placements(metrics, layout, ready.area);
    if let Some(glyphs) = metrics.path(&lines, layout.size, layout.width_axis, canvas.scale)? {
        render::paint(&mut canvas, &glyphs, &colours)?;
    }
    for index in 0..item.plates.len() {
        let plate = &item.plates[index];
        let (width, height) = (plate.rect.width, plate.rect.height);
        let lettering_at = |shift: [f64; 2]| {
            let quad = containers::plate_quad_at(ready.quad, plate, shift);
            warp::warp(&canvas, ready.area, quad, width, height)
        };
        let Some(lettering) = lettering_at(plate.shift) else {
            return Ok(Err(UNMAPPED.to_string()));
        };
        let background = plate_image(root, plate.plate.as_deref(), plate.rect, &item.id)?;
        let mask = mask_image(root, &plate.mask, plate.rect)?;
        let composed = DynamicImage::ImageRgba8(patch::patch(&background, &mask, &lettering));
        let relative = folder.join(format!("{index}.png"));
        patch::write_png(&root.join(&relative), &composed)?;
        let mut shifted = Vec::new();
        for (n, shift) in motion
            .other_shifts(&item.id, index, plate.shift)
            .into_iter()
            .enumerate()
        {
            let Some(lettering) = lettering_at(shift) else {
                return Ok(Err(UNMAPPED.to_string()));
            };
            let moved = DynamicImage::ImageRgba8(patch::patch(&background, &mask, &lettering));
            let path = folder.join(format!("{index}-{}.png", n + 1));
            patch::write_png(&root.join(&path), &moved)?;
            shifted.push(ShiftedPatch { shift, patch: path });
        }
        if index == key {
            let source = rgb_image(root, &plate.source, plate.rect)?;
            let preview = folder.join("preview.png");
            patch::write_png(
                &root.join(&preview),
                &DynamicImage::ImageRgb8(patch::preview(
                    composed.as_rgba8().ok_or("The patch is not RGBA")?,
                    &source,
                )),
            )?;
            item.preview = Some(preview);
        }
        item.plates[index].patch = Some(relative);
        item.plates[index].shifted = shifted;
    }
    item.status = ReplaceStatus::Baked;
    Ok(Ok(()))
}

/// Record a fallback: the reason, no patches, no preview and no patch folder.
pub(super) fn fall_back(root: &Path, item: &mut ReplacedText, reason: String) -> TextResult<()> {
    item.status = ReplaceStatus::Fallback(reason);
    item.preview = None;
    for plate in &mut item.plates {
        plate.patch = None;
        plate.shifted.clear();
    }
    reset(&root.join(folder(&item.id)))
}

/// The patch folder of an occurrence, relative to the job directory.
pub(super) fn folder(id: &str) -> PathBuf {
    let name: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let name = if name.is_empty() {
        "text".to_string()
    } else {
        name
    };
    Path::new(PATCHES).join(name)
}

fn reset(folder: &Path) -> TextResult<()> {
    match std::fs::remove_dir_all(folder) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Cannot clear {}: {e}", folder.display()).into()),
    }
}

fn plate_image(
    root: &Path,
    path: Option<&Path>,
    rect: PixelRect,
    id: &str,
) -> TextResult<RgbImage> {
    let path = path.ok_or_else(|| format!("{id}: a plate of this writing was not inpainted"))?;
    rgb_image(root, path, rect)
}

fn rgb_image(root: &Path, path: &Path, rect: PixelRect) -> TextResult<RgbImage> {
    let image = open(root, path)?.into_rgb8();
    sized(image.dimensions(), rect, path)?;
    Ok(image)
}

fn mask_image(root: &Path, path: &Path, rect: PixelRect) -> TextResult<GrayImage> {
    let image = open(root, path)?.into_luma8();
    sized(image.dimensions(), rect, path)?;
    Ok(image)
}

fn open(root: &Path, path: &Path) -> TextResult<DynamicImage> {
    let full = root.join(path);
    image::open(&full).map_err(|e| format!("Cannot read {}: {e}", full.display()).into())
}

fn sized(dimensions: (u32, u32), rect: PixelRect, path: &Path) -> TextResult<()> {
    if dimensions == (rect.width, rect.height) {
        Ok(())
    } else {
        Err(format!(
            "{} is {}×{} but its plate is {}×{}",
            path.display(),
            dimensions.0,
            dimensions.1,
            rect.width,
            rect.height
        )
        .into())
    }
}
