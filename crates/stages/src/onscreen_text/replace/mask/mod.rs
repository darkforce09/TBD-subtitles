//! Stroke masks, per-frame placement and background plates of translated writing.
//!
//! **Role:** decide for every displayable translated occurrence whether its strokes can be
//! erased, measure its lettering style, and collect the plates later steps inpaint and letter.
//! **Position:** the first replacement step, after text review; decodes only the keyframe plate
//! and the span's plate region of each occurrence through a [`RegionSource`].
//! **Signals and state:** `visual/masks/<occurrence>/` holds `mask.png`, `mask-<n>.png` for moved
//! placements and `source-<n>.png` per plate; the folder is emptied at the start of every run.
//! **Invariants:** one `ReplacedText` per candidate, in document order; visual problems fall
//! back with a reason while decode and file errors fail the step; the document validates.

mod cluster;
mod correlation;
mod files;
mod follow;
mod ink;
mod plates;
mod reach;
mod segment;
mod select;
mod style;

use std::path::Path;

use image::RgbImage;
use job_model::onscreen::{
    LetteringStyle, PixelRect, Plate, Point, ReplaceStatus, ReplacedText, ReplacementDocument,
    TextDocument, TextOccurrence,
};

use super::RegionSource;
use crate::onscreen_text::TextResult;
use files::{Folder, Names};
use plates::KeyPlate;

/// A per-occurrence verdict: the value, or why the occurrence stays in the subtitle file.
type Outcome<T> = Result<T, &'static str>;

/// Why an occurrence the owner placed nearby is not replaced.
const NEARBY: &str = "Nearby placement was chosen in Check Text";
/// Why an occurrence between two frames is not replaced.
const NO_FRAMES: &str = "No video frame starts while the writing is shown";
/// Why an occurrence without a usable quad is not replaced.
const NO_POSITION: &str = "The writing's position in the frame is unknown";

/// Measure every translated occurrence's strokes and plates; the document's frame count is the
/// source timeline's length.
pub fn extract(
    text: &TextDocument,
    source: &mut dyn RegionSource,
    root: &Path,
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
    let mut texts = Vec::with_capacity(candidates.len());
    for (done, occurrence) in candidates.iter().enumerate() {
        texts.push(replace(occurrence, source, root, &mut names)?);
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
    };
    let reason = if span.is_none() {
        Some(NO_FRAMES)
    } else if select::nearby(occurrence) {
        Some(NEARBY)
    } else {
        None
    };
    if let Some(reason) = reason {
        item.status = ReplaceStatus::Fallback(reason.to_string());
        return Ok(item);
    }
    match measure(occurrence, source, (first, last), root, names)? {
        Ok((style, plates)) => {
            item.style = Some(style);
            item.plates = plates;
        }
        Err(reason) => item.status = ReplaceStatus::Fallback(reason.to_string()),
    }
    Ok(item)
}

/// Segment the keyframe and collect the span's plates.
fn measure(
    occurrence: &TextOccurrence,
    source: &mut dyn RegionSource,
    span: (u64, u64),
    root: &Path,
    names: &mut Names,
) -> TextResult<Outcome<(LetteringStyle, Vec<Plate>)>> {
    let (width, height) = source.frame_size();
    let Some(keyframe) = occurrence.keyframe.as_ref() else {
        return Ok(Err(NO_POSITION));
    };
    let key_index = select::frame_at(source.timeline(), keyframe.time_s)
        .unwrap_or(span.0)
        .clamp(span.0, span.1);
    let Some(quad) =
        select::keyframe_quad(&occurrence.frames, keyframe.time_s).filter(|q| q.valid())
    else {
        return Ok(Err(NO_POSITION));
    };
    let margin = select::context_margin(quad);
    let window = select::around(quad, f64::from(select::ANALYSIS_MARGIN), width, height);
    let plate_rect = select::around(quad, margin, width, height);
    let (Some(window), Some(plate_rect)) = (window, plate_rect) else {
        return Ok(Err(NO_POSITION));
    };
    let key_crop = decode_one(source, plate_rect, key_index)?;
    let line_height = select::line_height(quad, &occurrence.japanese);
    let segmentation = match segment::segment(&key_crop, plate_rect, window, quad, line_height) {
        Ok(segmentation) => segmentation,
        Err(reason) => return Ok(Err(reason)),
    };
    let tracker = if select::is_static(&occurrence.frames, quad) {
        None
    } else {
        match follow::Tracker::new(
            &key_crop,
            plate_rect,
            window,
            &occurrence.frames,
            keyframe.time_s,
            select::shorter_side(quad),
        ) {
            Some(tracker) => Some(tracker),
            None => return Ok(Err(follow::UNFOLLOWED)),
        }
    };
    let key = KeyPlate {
        rect: plate_rect,
        mask: segmentation.mask,
        centre: Point {
            x: f64::from(window.x) + f64::from(window.width) / 2.0,
            y: f64::from(window.y) + f64::from(window.height) / 2.0,
        },
    };
    let folder = Folder::create(root, &names.claim(&occurrence.id))?;
    let outcome = match &tracker {
        None => plates::still(source, key, span, &folder)?,
        Some(tracker) => plates::moving(source, tracker, key, span, &folder)?,
    };
    match outcome {
        Ok(plates) => Ok(Ok((segmentation.style, plates))),
        Err(reason) => {
            folder.remove()?;
            Ok(Err(reason))
        }
    }
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
