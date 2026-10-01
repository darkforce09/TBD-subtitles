//! Background runs: consecutive frames that share one plate, collected one frame at a time.
//!
//! **Role:** place each frame's erase mask where the writing sits, keep the frame in the current
//! run while its mask overlaps the run's erase by [`MIN_MASK_IOU`] and the background around the
//! strokes stays the run's first frame's, and close the run otherwise; a run's plate is the union
//! of its frames' rectangles and its erase mask the union of their masks.
//! **Position:** fed by `plates::still` and `plates::moving`; its plates are inpainted and
//! lettered by later steps, its per-frame log becomes the occurrence's `frames` rows.
//! **Signals and state:** the current run's first-frame pixels, its union mask and placements; a
//! compact log of every frame's placement, correlation and plate; each closed run's source
//! pixels and mask are written to `source-<n>.png` and `mask.png` / `mask-<n>.png`.
//! **Invariants:** plates follow each other without overlap inside the occurrence's span; a run
//! keeps one scale; writing that never leaves one placement over one rectangle gives exactly the
//! plates and files a placement-per-run split gives; more than 2,000 runs falls back instead of
//! writing more.

use std::collections::HashMap;
use std::path::PathBuf;

use image::imageops;
use image::{GrayImage, Luma, RgbImage};
use job_model::onscreen::{PixelRect, Plate, Point, Quad};

use super::files::Folder;
use super::follow::Placement;
use super::select::bounding;
use super::{Outcome, TextResult};

/// Why an occurrence over a restless background stays in the subtitle file.
pub(super) const TOO_OFTEN: &str = "The background changes too often to repaint";
/// Most plates one occurrence may have.
const MAX_PLATES: usize = 2000;
/// A frame shares its run's plate while the unmasked plate pixels differ from the run's first
/// frame by less than this mean and this 99th percentile, per RGB channel value.
const MEAN_CHANGE: f64 = 3.0;
const P99_CHANGE: usize = 24;
/// Least intersection over union of a frame's erase mask with its run's union mask for the frame
/// to stay in the run. Every frame of a run then covers at least this share of the plate's erase,
/// so LaMa never repaints more than 1 / 0.85 ≈ 1.18 times a frame's own strokes: a one-pixel
/// jitter of a dilated stroke mask stays in its run, while writing that travels a few pixels
/// starts a new plate whose erase follows it.
pub(super) const MIN_MASK_IOU: f64 = 0.85;

/// The keyframe's plate: where it sits, its erase mask, the origin of placement scales and the
/// writing's tracked quad there.
pub(super) struct KeyPlate {
    pub rect: PixelRect,
    pub mask: GrayImage,
    pub centre: Point,
    pub quad: Quad,
}

impl KeyPlate {
    /// The keyframe mask moved and scaled onto `rect` for `placement`.
    pub(super) fn mask_at(&self, placement: Placement, rect: PixelRect) -> GrayImage {
        if placement == Placement::KEY && rect == self.rect {
            return self.mask.clone();
        }
        let s = placement.factor();
        let c = self.centre;
        let key = self.rect;
        GrayImage::from_fn(rect.width, rect.height, |u, v| {
            let x = f64::from(rect.x + u) + 0.5;
            let y = f64::from(rect.y + v) + 0.5;
            let kx = c.x + (x - c.x - f64::from(placement.dx)) / s - f64::from(key.x);
            let ky = c.y + (y - c.y - f64::from(placement.dy)) / s - f64::from(key.y);
            if kx < 0.0 || ky < 0.0 || kx >= f64::from(key.width) || ky >= f64::from(key.height) {
                Luma([0])
            } else {
                *self.mask.get_pixel(kx as u32, ky as u32)
            }
        })
    }

    /// The keyframe quad carried to `placement`: scaled about the keyframe window's centre and
    /// shifted, as the mask is.
    pub(super) fn quad_at(&self, placement: Placement) -> Quad {
        let s = placement.factor();
        let c = self.centre;
        Quad(self.quad.0.map(|p| Point {
            x: c.x + f64::from(placement.dx) + (p.x - c.x) * s,
            y: c.y + f64::from(placement.dy) + (p.y - c.y) * s,
        }))
    }
}

/// One frame of the span as the runs placed it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Placed {
    pub placement: Placement,
    pub rect: PixelRect,
    pub score: f32,
    pub plate: u32,
}

/// What the runs collected: the plates, every frame's place in frame order, and the keyframe plate
/// the frames' masks are carried from.
pub(super) struct Collected {
    pub plates: Vec<Plate>,
    pub placed: Vec<Placed>,
    pub key: KeyPlate,
}

/// The run being collected.
struct Current {
    /// Every placement the run's frames take, the first frame's first.
    placements: Vec<Placement>,
    rect: PixelRect,
    /// The union of the frames' masks over `rect`.
    union: GrayImage,
    /// The first frame's pixels of `region`, which holds every rectangle of the span.
    first_pixels: RgbImage,
    region: PixelRect,
    first: u64,
    last: u64,
}

/// A hashable copy of a rectangle.
type RectKey = (u32, u32, u32, u32);

fn key_of(rect: PixelRect) -> RectKey {
    (rect.x, rect.y, rect.width, rect.height)
}

/// Plates collected so far, the run in progress and every frame's place.
pub(super) struct Runs<'a> {
    folder: &'a Folder,
    pub(super) key: KeyPlate,
    plates: Vec<Plate>,
    current: Option<Current>,
    masks: HashMap<(Vec<Placement>, RectKey), PathBuf>,
    placed: Vec<Placed>,
    last_mask: Option<(Placement, PixelRect, GrayImage)>,
    overflow: bool,
}

impl<'a> Runs<'a> {
    pub(super) fn new(
        folder: &'a Folder,
        key: KeyPlate,
        frame: (u32, u32),
    ) -> TextResult<Runs<'a>> {
        if !key.rect.inside(frame.0, frame.1) {
            return Err("the keyframe plate lies outside the frame".into());
        }
        let path = folder.write("mask.png", &key.mask)?;
        let mut masks = HashMap::new();
        masks.insert((vec![Placement::KEY], key_of(key.rect)), path);
        Ok(Runs {
            folder,
            key,
            plates: Vec::new(),
            current: None,
            masks,
            placed: Vec::new(),
            last_mask: None,
            overflow: false,
        })
    }

    /// Add frame `index`, whose pixels of `region` are `pixels`, with the writing at `placement`
    /// on `rect` and correlated by `score`.
    pub(super) fn offer(
        &mut self,
        index: u64,
        pixels: RgbImage,
        region: PixelRect,
        placed: (Placement, PixelRect, f32),
    ) -> TextResult<()> {
        if self.overflow {
            return Ok(());
        }
        let (placement, rect, score) = placed;
        let mask = cached_mask(&self.key, &mut self.last_mask, placement, rect);
        if let Some(current) = self.current.as_mut()
            && joins(current, &pixels, region, placement, rect, mask)
        {
            absorb(current, placement, rect, mask);
            current.last = index;
            self.log(placement, rect, score);
            return Ok(());
        }
        let mask = mask.clone();
        if let Some(run) = self.current.take() {
            let plate = self.close(run)?;
            self.plates.push(plate);
        }
        if self.plates.len() >= MAX_PLATES {
            self.overflow = true;
            return Ok(());
        }
        self.current = Some(Current {
            placements: vec![placement],
            rect,
            union: mask,
            first_pixels: pixels,
            region,
            first: index,
            last: index,
        });
        self.log(placement, rect, score);
        Ok(())
    }

    fn log(&mut self, placement: Placement, rect: PixelRect, score: f32) {
        self.placed.push(Placed {
            placement,
            rect,
            score,
            plate: self.plates.len() as u32,
        });
    }

    /// Write the run's mask, unless a run of the same placements and rectangle wrote it, and its
    /// first frame's source pixels; its plate.
    fn close(&mut self, run: Current) -> TextResult<Plate> {
        let key = (run.placements.clone(), key_of(run.rect));
        let mask = match self.masks.get(&key) {
            Some(path) => path.clone(),
            None => {
                let name = format!("mask-{}.png", self.masks.len());
                let path = self.folder.write(&name, &run.union)?;
                self.masks.insert(key, path.clone());
                path
            }
        };
        let crop = if run.rect == run.region {
            run.first_pixels
        } else {
            imageops::crop_imm(
                &run.first_pixels,
                run.rect.x - run.region.x,
                run.rect.y - run.region.y,
                run.rect.width,
                run.rect.height,
            )
            .to_image()
        };
        let source = self
            .folder
            .write(&format!("source-{}.png", self.plates.len()), &crop)?;
        let first = run.placements[0];
        Ok(Plate {
            first_frame: run.first,
            last_frame: run.last,
            rect: run.rect,
            shift: [f64::from(first.dx), f64::from(first.dy)],
            scale: first.factor(),
            source,
            mask,
            plate: None,
            patch: None,
            shifted: Vec::new(),
        })
    }

    /// The plates, and every frame's place in order; the fallback reason when there were too many
    /// runs.
    pub(super) fn finish(mut self) -> TextResult<Outcome<Collected>> {
        if self.overflow {
            return Ok(Err(TOO_OFTEN));
        }
        if let Some(run) = self.current.take() {
            let plate = self.close(run)?;
            self.plates.push(plate);
        }
        Ok(Ok(Collected {
            plates: self.plates,
            placed: self.placed,
            key: self.key,
        }))
    }
}

/// Whether a frame with the writing at `placement` on `rect`, masked by `mask`, stays in
/// `current`: the same scale, its mask overlapping the run's union by [`MIN_MASK_IOU`], and the
/// background its first frame showed.
fn joins(
    current: &Current,
    pixels: &RgbImage,
    region: PixelRect,
    placement: Placement,
    rect: PixelRect,
    mask: &GrayImage,
) -> bool {
    if region != current.region || placement.scale != current.placements[0].scale {
        return false;
    }
    let same_place = current.placements == [placement] && rect == current.rect;
    if !same_place && mask_iou(mask, rect, &current.union, current.rect) < MIN_MASK_IOU {
        return false;
    }
    if same_place && rect == region {
        return same_background(&current.first_pixels, pixels, &current.union);
    }
    let crop = |image: &RgbImage| {
        imageops::crop_imm(
            image,
            rect.x - region.x,
            rect.y - region.y,
            rect.width,
            rect.height,
        )
        .to_image()
    };
    let excluded = GrayImage::from_fn(rect.width, rect.height, |u, v| {
        let (x, y) = (rect.x + u, rect.y + v);
        let in_union = holds(current.rect, x, y)
            && current
                .union
                .get_pixel(x - current.rect.x, y - current.rect.y)
                .0[0]
                != 0;
        Luma([u8::from(in_union || mask.get_pixel(u, v).0[0] != 0) * u8::MAX])
    });
    same_background(&crop(&current.first_pixels), &crop(pixels), &excluded)
}

/// The mask of `placement` on `rect`, from `cache` when the last frame's was the same.
fn cached_mask<'m>(
    key: &KeyPlate,
    cache: &'m mut Option<(Placement, PixelRect, GrayImage)>,
    placement: Placement,
    rect: PixelRect,
) -> &'m GrayImage {
    if !matches!(cache, Some((p, r, _)) if *p == placement && *r == rect) {
        *cache = None;
    }
    &cache
        .get_or_insert_with(|| (placement, rect, key.mask_at(placement, rect)))
        .2
}

/// Grow `current` by a frame at `placement` on `rect` with `mask`; a placement the run already
/// holds sits on the same rectangle and its mask is already in the union.
fn absorb(current: &mut Current, placement: Placement, rect: PixelRect, mask: &GrayImage) {
    if current.placements.contains(&placement) {
        return;
    }
    current.placements.push(placement);
    let grown = bounding(current.rect, rect);
    if grown != current.rect {
        let mut union = GrayImage::new(grown.width, grown.height);
        imageops::replace(
            &mut union,
            &current.union,
            i64::from(current.rect.x - grown.x),
            i64::from(current.rect.y - grown.y),
        );
        current.union = union;
        current.rect = grown;
    }
    let (ox, oy) = (rect.x - current.rect.x, rect.y - current.rect.y);
    for (u, v, value) in mask.enumerate_pixels() {
        if value.0[0] != 0 {
            current.union.put_pixel(ox + u, oy + v, *value);
        }
    }
}

/// Whether pixel `(x, y)` lies in `rect`.
fn holds(rect: PixelRect, x: u32, y: u32) -> bool {
    (rect.x..rect.right()).contains(&x) && (rect.y..rect.bottom()).contains(&y)
}

/// Intersection over union of the non-zero pixels of mask `a` on `a_rect` and `b` on `b_rect`.
pub(super) fn mask_iou(a: &GrayImage, a_rect: PixelRect, b: &GrayImage, b_rect: PixelRect) -> f64 {
    let on = |mask: &GrayImage, rect: PixelRect, x: u32, y: u32| {
        holds(rect, x, y) && mask.get_pixel(x - rect.x, y - rect.y).0[0] != 0
    };
    let span = bounding(a_rect, b_rect);
    let (mut shared, mut either) = (0u64, 0u64);
    for y in span.y..span.bottom() {
        for x in span.x..span.right() {
            let (in_a, in_b) = (on(a, a_rect, x, y), on(b, b_rect, x, y));
            shared += u64::from(in_a && in_b);
            either += u64::from(in_a || in_b);
        }
    }
    if either == 0 {
        1.0
    } else {
        shared as f64 / either as f64
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
#[path = "tests/runs.rs"]
mod tests;
