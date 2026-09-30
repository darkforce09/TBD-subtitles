//! Background runs: consecutive frames that share one plate.
//!
//! **Role:** stream an occurrence's frames once, place the erase mask on each, and start a new
//! plate whenever the writing moves or the background around the strokes changes.
//! **Position:** after segmentation, and after following for moving writing; its plates are
//! inpainted and lettered by later steps.
//! **Signals and state:** only the current run's first-frame pixels and mask are held; each
//! run's source pixels are written to `source-<n>.png` as it starts.
//! **Invariants:** plates follow each other without overlap inside the occurrence's span; a run
//! never mixes placements; more than 2,000 runs falls back instead of writing more.

use std::collections::HashMap;
use std::path::PathBuf;

use image::imageops;
use image::{GrayImage, Luma, RgbImage};
use job_model::onscreen::{PixelRect, Plate, Point};

use super::files::Folder;
use super::follow::{self, Placement, Tracker, UNFOLLOWED};
use super::select::{bounding, clamp};
use super::{Outcome, RegionSource, check_size};
use crate::onscreen_text::TextResult;

/// Why an occurrence over a restless background stays in the subtitle file.
pub(super) const TOO_OFTEN: &str = "The background changes too often to repaint";
/// Most plates one occurrence may have.
const MAX_PLATES: usize = 2000;
/// A frame shares its run's plate while the unmasked plate pixels differ from the run's first
/// frame by less than this mean and this 99th percentile, per RGB channel value.
const MEAN_CHANGE: f64 = 3.0;
const P99_CHANGE: usize = 24;
/// Largest share of the frame the plates of moving writing may sweep.
const MAX_UNION_SHARE: f64 = 0.25;

/// The keyframe's plate: where it sits, its erase mask, and the origin of placement scales.
pub(super) struct KeyPlate {
    pub rect: PixelRect,
    pub mask: GrayImage,
    pub centre: Point,
}

/// Collect the runs of writing that stays where the keyframe shows it.
pub(super) fn still(
    source: &mut dyn RegionSource,
    key: KeyPlate,
    span: (u64, u64),
    folder: &Folder,
) -> TextResult<Outcome<Vec<Plate>>> {
    let rect = key.rect;
    let mut runs = Runs::new(folder, key, source.frame_size())?;
    source.frames(rect, span.0, span.1, &mut |index, image| {
        check_size(&image, rect)?;
        runs.offer(index, image, Placement::KEY, rect)
    })?;
    Ok(runs.finish())
}

/// Follow moving writing through its span, then collect its runs from one decode of the
/// region its plates sweep.
pub(super) fn moving(
    source: &mut dyn RegionSource,
    tracker: &Tracker,
    key: KeyPlate,
    span: (u64, u64),
    folder: &Folder,
) -> TextResult<Outcome<Vec<Plate>>> {
    let placements = match follow::follow(source, tracker, span)? {
        Ok(placements) => placements,
        Err(reason) => return Ok(Err(reason)),
    };
    let frame = source.frame_size();
    let Some(rects) = placements
        .iter()
        .map(|p| placed(&key, frame, *p))
        .collect::<Option<Vec<PixelRect>>>()
    else {
        return Ok(Err(UNFOLLOWED));
    };
    let Some(union) = rects.iter().copied().reduce(bounding) else {
        return Ok(Err(UNFOLLOWED));
    };
    if union.area() as f64 > MAX_UNION_SHARE * f64::from(frame.0) * f64::from(frame.1) {
        return Ok(Err(UNFOLLOWED));
    }
    let mut runs = Runs::new(folder, key, frame)?;
    source.frames(union, span.0, span.1, &mut |index, image| {
        check_size(&image, union)?;
        let i = index.wrapping_sub(span.0) as usize;
        let (Some(&placement), Some(&rect)) = (placements.get(i), rects.get(i)) else {
            return Err(format!("frame {index} is outside the requested span").into());
        };
        let crop = imageops::crop_imm(
            &image,
            rect.x - union.x,
            rect.y - union.y,
            rect.width,
            rect.height,
        )
        .to_image();
        runs.offer(index, crop, placement, rect)
    })?;
    Ok(runs.finish())
}

/// Where the plate sits for `placement`, clipped to the frame.
fn placed(key: &KeyPlate, frame: (u32, u32), placement: Placement) -> Option<PixelRect> {
    let rect = key.rect;
    let (dx, dy) = (i64::from(placement.dx), i64::from(placement.dy));
    if placement.scale == Placement::KEY.scale {
        return clamp(
            i64::from(rect.x) + dx,
            i64::from(rect.y) + dy,
            i64::from(rect.right()) + dx,
            i64::from(rect.bottom()) + dy,
            frame,
        );
    }
    let s = placement.factor();
    let c = key.centre;
    let map_x = |x: u32| c.x + dx as f64 + (f64::from(x) - c.x) * s;
    let map_y = |y: u32| c.y + dy as f64 + (f64::from(y) - c.y) * s;
    clamp(
        map_x(rect.x).floor() as i64,
        map_y(rect.y).floor() as i64,
        map_x(rect.right()).ceil() as i64,
        map_y(rect.bottom()).ceil() as i64,
        frame,
    )
}

/// The run being collected.
struct Current {
    placement: Placement,
    rect: PixelRect,
    mask: GrayImage,
    mask_path: PathBuf,
    reference: RgbImage,
    source: PathBuf,
    first: u64,
    last: u64,
}

/// A hashable copy of a rectangle.
type RectKey = (u32, u32, u32, u32);

fn key_of(rect: PixelRect) -> RectKey {
    (rect.x, rect.y, rect.width, rect.height)
}

/// Plates collected so far and the run in progress.
struct Runs<'a> {
    folder: &'a Folder,
    key: KeyPlate,
    plates: Vec<Plate>,
    current: Option<Current>,
    masks: HashMap<(Placement, RectKey), PathBuf>,
    overflow: bool,
}

impl<'a> Runs<'a> {
    fn new(folder: &'a Folder, key: KeyPlate, frame: (u32, u32)) -> TextResult<Runs<'a>> {
        if !key.rect.inside(frame.0, frame.1) {
            return Err("the keyframe plate lies outside the frame".into());
        }
        let path = folder.write("mask.png", &key.mask)?;
        let mut masks = HashMap::new();
        masks.insert((Placement::KEY, key_of(key.rect)), path);
        Ok(Runs {
            folder,
            key,
            plates: Vec::new(),
            current: None,
            masks,
            overflow: false,
        })
    }

    /// The keyframe mask moved and scaled onto `rect` for `placement`.
    fn mask_at(&self, placement: Placement, rect: PixelRect) -> GrayImage {
        if placement == Placement::KEY && rect == self.key.rect {
            return self.key.mask.clone();
        }
        let s = placement.factor();
        let c = self.key.centre;
        let key = self.key.rect;
        GrayImage::from_fn(rect.width, rect.height, |u, v| {
            let x = f64::from(rect.x + u) + 0.5;
            let y = f64::from(rect.y + v) + 0.5;
            let kx = c.x + (x - c.x - f64::from(placement.dx)) / s - f64::from(key.x);
            let ky = c.y + (y - c.y - f64::from(placement.dy)) / s - f64::from(key.y);
            if kx < 0.0 || ky < 0.0 || kx >= f64::from(key.width) || ky >= f64::from(key.height) {
                Luma([0])
            } else {
                *self.key.mask.get_pixel(kx as u32, ky as u32)
            }
        })
    }

    /// Add frame `index`, showing `crop` at `rect` with the writing at `placement`.
    fn offer(
        &mut self,
        index: u64,
        crop: RgbImage,
        placement: Placement,
        rect: PixelRect,
    ) -> TextResult<()> {
        if self.overflow {
            return Ok(());
        }
        if let Some(current) = self.current.as_mut()
            && current.placement == placement
            && current.rect == rect
            && same_background(&current.reference, &crop, &current.mask)
        {
            current.last = index;
            return Ok(());
        }
        let previous = self.current.take();
        if let Some(run) = &previous {
            self.plates.push(plate(run));
        }
        if self.plates.len() >= MAX_PLATES {
            self.overflow = true;
            return Ok(());
        }
        let (mask, mask_path) = match previous {
            Some(run) if run.placement == placement && run.rect == rect => {
                (run.mask, run.mask_path)
            }
            _ => {
                let mask = self.mask_at(placement, rect);
                let path = match self.masks.get(&(placement, key_of(rect))) {
                    Some(path) => path.clone(),
                    None => {
                        let name = format!("mask-{}.png", self.masks.len());
                        let path = self.folder.write(&name, &mask)?;
                        self.masks.insert((placement, key_of(rect)), path.clone());
                        path
                    }
                };
                (mask, path)
            }
        };
        let source = self
            .folder
            .write(&format!("source-{}.png", self.plates.len()), &crop)?;
        self.current = Some(Current {
            placement,
            rect,
            mask,
            mask_path,
            reference: crop,
            source,
            first: index,
            last: index,
        });
        Ok(())
    }

    fn finish(mut self) -> Outcome<Vec<Plate>> {
        if self.overflow {
            return Err(TOO_OFTEN);
        }
        if let Some(run) = self.current.take() {
            self.plates.push(plate(&run));
        }
        Ok(self.plates)
    }
}

fn plate(run: &Current) -> Plate {
    Plate {
        first_frame: run.first,
        last_frame: run.last,
        rect: run.rect,
        shift: [f64::from(run.placement.dx), f64::from(run.placement.dy)],
        scale: run.placement.factor(),
        source: run.source.clone(),
        mask: run.mask_path.clone(),
        plate: None,
        patch: None,
    }
}

/// Whether the pixels the mask keeps look the same in both crops.
pub(super) fn same_background(reference: &RgbImage, frame: &RgbImage, mask: &GrayImage) -> bool {
    let mut histogram = [0usize; 256];
    let mut total = 0usize;
    let mut sum = 0u64;
    for ((a, b), m) in reference.pixels().zip(frame.pixels()).zip(mask.pixels()) {
        if m.0[0] != 0 {
            continue;
        }
        for (x, y) in a.0.iter().zip(&b.0) {
            let d = x.abs_diff(*y);
            histogram[usize::from(d)] += 1;
            sum += u64::from(d);
            total += 1;
        }
    }
    if total == 0 {
        return true;
    }
    let mut seen = 0usize;
    let mut p99 = usize::from(u8::MAX);
    for (value, &n) in histogram.iter().enumerate() {
        seen += n;
        if seen as f64 >= 0.99 * total as f64 {
            p99 = value;
            break;
        }
    }
    (sum as f64 / total as f64) < MEAN_CHANGE && p99 < P99_CHANGE
}

#[cfg(test)]
#[path = "tests/plates.rs"]
mod tests;
