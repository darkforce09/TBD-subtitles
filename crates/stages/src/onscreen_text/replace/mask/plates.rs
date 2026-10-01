//! An occurrence's frames streamed once into background runs, and its per-frame rows.
//!
//! **Role:** follow the writing through its span (still writing stays at the keyframe
//! placement, moving writing at the placement the tracker found), hand every frame to the runs
//! with its correlation, and once the occurrence has its plates, send one `FrameRecord` per frame:
//! the quad, correlation, shift and scale of its placement, its erase mask as run-length rows
//! relative to its plate, and its plate.
//! **Position:** after segmentation, and after following for moving writing; called by
//! `mask::measure`; the plates are inpainted and lettered by later steps, the rows are the job's
//! `frames` table.
//! **Signals and state:** one decoded region per frame at a time; the runs' compact log of every
//! frame's placement; one frame mask and its run-length rows at a time while the rows are sent.
//! **Invariants:** every frame of the span gets exactly one row, in frame order, and only when the
//! occurrence keeps its plates; a row's mask is the keyframe mask carried to the frame's
//! placement, inside its plate's rectangle; still writing is decoded over the keyframe plate
//! alone.

use image::imageops;
use image::{GrayImage, RgbImage};
use job_model::onscreen::{FrameRecord, PixelRect, Plate, encode_mask};

use super::correlation::{Integral, Plane, Template, grey};
use super::files::Folder;
use super::follow::{self, Path, Placement, Tracker, UNFOLLOWED};
use super::runs::{Collected, KeyPlate, Placed, Runs};
use super::select::{bounding, clamp};
use super::{FrameSink, Outcome, RegionSource, check_size};
use crate::onscreen_text::TextResult;

/// Largest share of the frame the plates of moving writing may sweep.
const MAX_UNION_SHARE: f64 = 0.25;

/// What one occurrence's plates are collected from.
pub(super) struct Span<'a> {
    pub id: &'a str,
    pub frames: (u64, u64),
    /// The keyframe window: the writing the correlation of still writing compares.
    pub window: PixelRect,
    /// The keyframe's pixels of the keyframe plate.
    pub key_pixels: &'a RgbImage,
}

/// Collect the runs of writing that stays where the keyframe shows it.
pub(super) fn still(
    source: &mut dyn RegionSource,
    key: KeyPlate,
    span: &Span,
    folder: &Folder,
    sink: FrameSink,
) -> TextResult<Outcome<Vec<Plate>>> {
    let rect = key.rect;
    let scorer = KeyScore::new(span.key_pixels, rect, span.window);
    let mut runs = Runs::new(folder, key, source.frame_size())?;
    source.frames(rect, span.frames.0, span.frames.1, &mut |index, image| {
        check_size(&image, rect)?;
        let score = scorer.as_ref().map_or(0.0, |scorer| scorer.score(&image));
        runs.offer(index, image, rect, (Placement::KEY, rect, score))
    })?;
    send(runs, span, sink)
}

/// Follow moving writing through its span, then collect its runs from one decode of the
/// region its plates sweep; writing the search finds still is collected as [`still`] writing.
pub(super) fn moving(
    source: &mut dyn RegionSource,
    tracker: &Tracker,
    key: KeyPlate,
    span: &Span,
    folder: &Folder,
    sink: FrameSink,
) -> TextResult<Outcome<Vec<Plate>>> {
    let placements = match follow::follow(source, tracker, span.frames)? {
        Ok(Path::Moving(placements)) => placements,
        Ok(Path::Still) => return still(source, key, span, folder, sink),
        Err(reason) => return Ok(Err(reason)),
    };
    let frame = source.frame_size();
    let Some(rects) = placements
        .iter()
        .map(|(p, _)| placed(&key, frame, *p))
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
    let first = span.frames.0;
    let mut runs = Runs::new(folder, key, frame)?;
    source.frames(union, first, span.frames.1, &mut |index, image| {
        check_size(&image, union)?;
        let i = index.wrapping_sub(first) as usize;
        let (Some(&(placement, score)), Some(&rect)) = (placements.get(i), rects.get(i)) else {
            return Err(format!("frame {index} is outside the requested span").into());
        };
        runs.offer(index, image, union, (placement, rect, score))
    })?;
    send(runs, span, sink)
}

/// The plates the runs collected, after one row per frame went to `sink`.
fn send(runs: Runs, span: &Span, sink: FrameSink) -> TextResult<Outcome<Vec<Plate>>> {
    let Collected {
        plates,
        placed,
        key,
    } = match runs.finish()? {
        Ok(found) => found,
        Err(reason) => return Ok(Err(reason)),
    };
    let mut rows = Rows::default();
    for (offset, frame) in placed.iter().enumerate() {
        let plate = plates
            .get(frame.plate as usize)
            .ok_or("a frame names a plate the runs did not keep")?;
        let record = rows.record(&key, frame, plate.rect)?;
        sink(span.id, span.frames.0 + offset as u64, record)?;
    }
    Ok(Ok(plates))
}

/// The row of the frame being sent, its run-length mask reused while the placement, the
/// rectangle and the plate stay the same.
#[derive(Default)]
struct Rows {
    at: Option<(Placement, PixelRect, PixelRect)>,
    record: FrameRecord,
}

impl Rows {
    fn record(
        &mut self,
        key: &KeyPlate,
        frame: &Placed,
        plate: PixelRect,
    ) -> TextResult<&FrameRecord> {
        let at = (frame.placement, frame.rect, plate);
        if self.at != Some(at) {
            let mask = key.mask_at(frame.placement, frame.rect);
            let on_plate = if frame.rect == plate {
                mask
            } else {
                let mut on_plate = GrayImage::new(plate.width, plate.height);
                imageops::replace(
                    &mut on_plate,
                    &mask,
                    i64::from(frame.rect.x) - i64::from(plate.x),
                    i64::from(frame.rect.y) - i64::from(plate.y),
                );
                on_plate
            };
            self.record.mask = encode_mask(plate.width, plate.height, on_plate.as_raw())
                .ok_or("a plate is too large for run-length mask rows")?;
            self.record.quad = key.quad_at(frame.placement);
            self.record.shift = [f64::from(frame.placement.dx), f64::from(frame.placement.dy)];
            self.record.scale = frame.placement.factor();
            self.at = Some(at);
        }
        self.record.follow_score = frame.score;
        self.record.plate = frame.plate;
        Ok(&self.record)
    }
}

/// The correlation of still writing with its keyframe: the keyframe window compared with the
/// same window of each frame.
struct KeyScore {
    template: Template,
    /// The window's offset inside the keyframe plate.
    offset: (u32, u32),
    size: (u32, u32),
}

impl KeyScore {
    /// The keyframe window of `key_pixels`, the keyframe's crop of `plate`; `None` when the
    /// window has no contrast to correlate or leaves the plate.
    fn new(key_pixels: &RgbImage, plate: PixelRect, window: PixelRect) -> Option<KeyScore> {
        if window.x < plate.x
            || window.y < plate.y
            || window.right() > plate.right()
            || window.bottom() > plate.bottom()
            || key_pixels.dimensions() != (plate.width, plate.height)
        {
            return None;
        }
        let offset = (window.x - plate.x, window.y - plate.y);
        let size = (window.width, window.height);
        let crop = imageops::crop_imm(key_pixels, offset.0, offset.1, size.0, size.1).to_image();
        let template = Template::new(&Plane::from_gray(&grey(&crop)))?;
        Some(KeyScore {
            template,
            offset,
            size,
        })
    }

    /// The correlation of the window of `pixels`, a frame's crop of the keyframe plate.
    fn score(&self, pixels: &RgbImage) -> f32 {
        let (x, y) = self.offset;
        let crop = imageops::crop_imm(pixels, x, y, self.size.0, self.size.1).to_image();
        let plane = Plane::from_gray(&grey(&crop));
        let integral = Integral::new(&plane);
        self.template.score(&plane, &integral, 0, 0)
    }
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
