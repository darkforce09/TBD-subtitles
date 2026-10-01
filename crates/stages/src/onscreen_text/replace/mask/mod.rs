//! Stroke masks, per-frame placement and background plates of translated writing.
//!
//! **Role:** decide for every displayable translated occurrence whether its strokes can be
//! erased, measure its lettering style, and collect the plates later steps inpaint and letter.
//! **Position:** the first replacement step, after text review; decodes only the keyframe plate
//! and the span's plate region of each occurrence through a [`RegionSource`].
//! **Signals and state:** `visual/masks/<occurrence>/` holds `mask.png`, `mask-<n>.png` for the
//! union mask of every other plate and `source-<n>.png` per plate; the folder is emptied at the
//! start of every run; each frame of an occurrence that keeps its plates goes to the caller's
//! `FrameSink` as one `FrameRecord`, the per-frame truth of where the erase sits.
//! **Invariants:** one `ReplacedText` per candidate, in document order; visual problems fall
//! back with a reason while decode and file errors fail the step; the document validates.

mod cluster;
mod complete;
mod correlation;
mod files;
mod follow;
mod ink;
mod panel;
mod pieces;
mod plates;
mod probe;
mod reach;
mod runs;
mod segment;
mod select;
mod style;

use std::collections::HashSet;
use std::path::Path;

use image::RgbImage;
use job_model::onscreen::{
    FrameRecord, LetteringStyle, PixelRect, Plate, Point, Quad, ReplaceStatus, ReplacedText,
    ReplacementDocument, TextDocument, TextOccurrence,
};

use super::RegionSource;
use crate::onscreen_text::TextResult;
use files::{Folder, Names};
use plates::Span;
pub use probe::{
    Completeness, Diagnosis, Followed, Following, Judgement, Reading, Trace, diagnose,
};
use runs::KeyPlate;
use select::Areas;

/// The lettering colour tolerance shared by completion and the residual check after inpainting.
pub(crate) use complete::INK_DELTA_E;

/// CIE76 ΔE between two sRGB colours.
pub(crate) fn delta_e(a: [u8; 3], b: [u8; 3]) -> f32 {
    cluster::distance(cluster::lab(a), cluster::lab(b))
}

/// A per-occurrence verdict: the value, or why the occurrence stays in the subtitle file.
type Outcome<T> = Result<T, &'static str>;

/// Why an occurrence the owner placed nearby is not replaced.
const NEARBY: &str = "Nearby placement was chosen in Check Text";
/// Why an occurrence between two frames is not replaced.
const NO_FRAMES: &str = "No video frame starts while the writing is shown";
/// Why an occurrence without a usable quad is not replaced.
const NO_POSITION: &str = "The writing's position in the frame is unknown";
/// Why an occurrence whose id an earlier one has is not replaced: its frame rows would collide.
const SAME_ID: &str = "Another occurrence has the same id";

/// Where the stroke-mask step sends one occurrence's per-frame row: its id, the frame and the row.
pub type FrameSink<'a> = &'a mut dyn FnMut(&str, u64, &FrameRecord) -> TextResult<()>;

/// Measure every translated occurrence's strokes and plates, and send each frame of an occurrence
/// that keeps its plates to `frames` as a `FrameRecord`, in frame order; the document's frame
/// count is the source timeline's length.
pub fn extract(
    text: &TextDocument,
    source: &mut dyn RegionSource,
    root: &Path,
    frames: FrameSink,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> TextResult<ReplacementDocument> {
    let (width, height) = source.frame_size();
    let frame_count = source.timeline().len() as u64;
    files::remove_tree(&root.join(files::MASKS))?;
    let candidates: Vec<&TextOccurrence> = text
        .occurrences
        .iter()
        .filter(|t| select::candidate(t))
        .collect();
    if !candidates.is_empty() && frame_count == 0 {
        return Err("the video has no decoded frames".into());
    }
    let mut names = Names::default();
    let mut with_rows = HashSet::new();
    let mut texts = Vec::with_capacity(candidates.len());
    for (done, occurrence) in candidates.iter().enumerate() {
        let item = replace(occurrence, source, root, &mut names, &with_rows, frames)?;
        if !item.plates.is_empty() {
            with_rows.insert(occurrence.id.clone());
        }
        texts.push(item);
        progress(done + 1, candidates.len());
    }
    let document = ReplacementDocument {
        width,
        height,
        frame_count,
        texts,
    };
    document.validate()?;
    Ok(document)
}

/// One occurrence's replacement, or its fallback.
fn replace(
    occurrence: &TextOccurrence,
    source: &mut dyn RegionSource,
    root: &Path,
    names: &mut Names,
    with_rows: &HashSet<String>,
    frames: FrameSink,
) -> TextResult<ReplacedText> {
    let timeline = source.timeline();
    let span = select::span(timeline, occurrence.start_s, occurrence.end_s);
    let (first, last) = span.unwrap_or_else(|| {
        let nearest = select::frame_at(timeline, occurrence.start_s).unwrap_or(0);
        (nearest, nearest)
    });
    let mut item = ReplacedText {
        id: occurrence.id.clone(),
        first_frame: first,
        last_frame: last,
        status: ReplaceStatus::Pending,
        style: None,
        container: None,
        plates: Vec::new(),
        preview: None,
        lettering_quad: None,
    };
    let reason = if span.is_none() {
        Some(NO_FRAMES)
    } else if select::nearby(occurrence) {
        Some(NEARBY)
    } else if with_rows.contains(&occurrence.id) {
        Some(SAME_ID)
    } else {
        None
    };
    if let Some(reason) = reason {
        item.status = ReplaceStatus::Fallback(reason.to_string());
        return Ok(item);
    }
    match measure(occurrence, source, (first, last), root, names, frames)? {
        Ok(measured) => {
            item.style = Some(measured.style);
            item.plates = measured.plates;
            item.lettering_quad = measured.lettering_quad;
        }
        Err(reason) => item.status = ReplaceStatus::Fallback(reason.to_string()),
    }
    Ok(item)
}

/// What the keyframe and the span's plates yield for a separable occurrence.
struct Measured {
    style: LetteringStyle,
    plates: Vec<Plate>,
    lettering_quad: Option<Quad>,
}

/// The keyframe index inside `span` and the rectangles measured around its quad.
fn locate(
    occurrence: &TextOccurrence,
    source: &dyn RegionSource,
    span: (u64, u64),
) -> Outcome<(u64, Areas)> {
    let (width, height) = source.frame_size();
    let keyframe = occurrence.keyframe.as_ref().ok_or(NO_POSITION)?;
    let key_index = select::frame_at(source.timeline(), keyframe.time_s)
        .unwrap_or(span.0)
        .clamp(span.0, span.1);
    let quad = select::keyframe_quad(&occurrence.frames, keyframe.time_s)
        .filter(|q| q.valid())
        .ok_or(NO_POSITION)?;
    let areas = select::areas(occurrence, quad, width, height).ok_or(NO_POSITION)?;
    Ok((key_index, areas))
}

/// Segment the keyframe and collect the span's plates.
fn measure(
    occurrence: &TextOccurrence,
    source: &mut dyn RegionSource,
    span: (u64, u64),
    root: &Path,
    names: &mut Names,
    frames: FrameSink,
) -> TextResult<Outcome<Measured>> {
    let (key_index, areas) = match locate(occurrence, source, span) {
        Ok(found) => found,
        Err(reason) => return Ok(Err(reason)),
    };
    let key_crop = decode_one(source, areas.plate, key_index)?;
    let segmentation = match separate(&key_crop, &areas, &mut Trace::default()) {
        Ok(segmentation) => segmentation,
        Err(reason) => return Ok(Err(reason)),
    };
    let tracker = match tracker(occurrence, &key_crop, &areas) {
        None => None,
        Some(Ok(tracker)) => Some(tracker),
        Some(Err(reason)) => return Ok(Err(reason)),
    };
    let window = areas.window;
    let key = KeyPlate {
        rect: areas.plate,
        mask: segmentation.mask,
        centre: Point {
            x: f64::from(window.x) + f64::from(window.width) / 2.0,
            y: f64::from(window.y) + f64::from(window.height) / 2.0,
        },
        quad: areas.quad,
    };
    let folder = Folder::create(root, &names.claim(&occurrence.id))?;
    let span = Span {
        id: &occurrence.id,
        frames: span,
        window,
        key_pixels: &key_crop,
    };
    let outcome = match &tracker {
        None => plates::still(source, key, &span, &folder, frames)?,
        Some(tracker) => plates::moving(source, tracker, key, &span, &folder, frames)?,
    };
    match outcome {
        Ok(plates) => Ok(Ok(Measured {
            style: segmentation.style,
            plates,
            lettering_quad: segmentation.lettering,
        })),
        Err(reason) => {
            folder.remove()?;
            Ok(Err(reason))
        }
    }
}

/// The tracker that follows moving writing from the keyframe plate `key_crop`; `None` for
/// static writing, and an error when the writing has no contrast to follow.
fn tracker(
    occurrence: &TextOccurrence,
    key_crop: &RgbImage,
    areas: &Areas,
) -> Option<Outcome<follow::Tracker>> {
    if select::is_static(&occurrence.frames, areas.quad) {
        return None;
    }
    let key_time_s = occurrence.keyframe.as_ref().map_or(0.0, |k| k.time_s);
    Some(
        follow::Tracker::new(
            key_crop,
            areas.plate,
            areas.window,
            &occurrence.frames,
            key_time_s,
            select::shorter_side(areas.quad),
        )
        .ok_or(follow::UNFOLLOWED),
    )
}

/// Segment the keyframe plate `crop`; a padded loose box that does not separate is tried again
/// within its unpadded window.
fn separate(
    crop: &RgbImage,
    areas: &Areas,
    trace: &mut Trace,
) -> Result<segment::Segmentation, &'static str> {
    segment::segment(crop, areas, trace).or_else(|reason| match areas.unpadded() {
        Some(unpadded) => segment::segment(crop, &unpadded, trace),
        None => Err(reason),
    })
}

/// Fail when the source hands back a region of another size than requested.
fn check_size(image: &RgbImage, rect: PixelRect) -> TextResult<()> {
    if image.dimensions() == (rect.width, rect.height) {
        return Ok(());
    }
    Err(format!(
        "decoded region is {}x{}, expected {}x{}",
        image.width(),
        image.height(),
        rect.width,
        rect.height
    )
    .into())
}

/// Decode `rect` of frame `index`.
fn decode_one(source: &mut dyn RegionSource, rect: PixelRect, index: u64) -> TextResult<RgbImage> {
    let mut decoded = None;
    source.frames(rect, index, index, &mut |_, image| {
        decoded = Some(image);
        Ok(())
    })?;
    match decoded {
        Some(image) if image.dimensions() == (rect.width, rect.height) => Ok(image),
        Some(_) => Err(format!("frame {index} decoded at the wrong size").into()),
        None => Err(format!("frame {index} could not be decoded").into()),
    }
}

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixtures;

#[cfg(test)]
#[path = "tests/extract.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/frame_rows.rs"]
mod frame_rows;
