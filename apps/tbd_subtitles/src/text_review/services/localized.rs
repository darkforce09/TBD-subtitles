//! A job's localized video as Check Text shows it: its files, each occurrence's replacement, the
//! selected occurrence's pictures, and the words and geometry the preview draws them with.
//!
//! **Role:** read `visual/localized_video.json`, `visual/text_verify.json` (else, in a job from
//! before the read-back check, `visual/text_compose.json`) and the localized subtitle file named
//! in `output.json`; find each occurrence's keyframe plate as composition chose it; decode its
//! replaced plate and erase mask at a bounded size; say whether it was replaced and what the
//! read-back check read; place a plate's rectangle on the picture.
//! **Position:** called by `session::load` and by application actions off the window thread; the
//! pure helpers are also read by the preview.
//! **Signals and state:** reads job files and PNGs; writes nothing.
//! **Invariants:** a job without the localized video yields nothing; a missing or broken record
//! or picture is never an error, only absent; paths from documents stay inside the job folder;
//! decoded pictures are at most `PICTURE_EDGE` pixels on a side.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::onscreen::{
    LocalizedVideoRecord, PixelRect, Plate, ReplaceStatus, ReplacedText, ReplacementDocument,
    TextCheck, TextDocument, TextOccurrence, VerifiedReplacements, VerifyReading,
};
use job_model::outputs::OutputRecord;
use pipeline::work_dir::{self, WorkDir};

use super::player;
use crate::text_review::models::{
    LocalizedReview, Mask, MaskPlate, PreviewMode, Replacement, ReplacementPictures,
};

/// The longest side of a decoded replaced plate or erase mask.
const PICTURE_EDGE: u32 = 720;

/// The localized video of the job in `work`, when its settings write one.
pub(crate) fn load(
    work: &WorkDir,
    job: &JobRecord,
    output: &OutputRecord,
    document: &TextDocument,
) -> Option<LocalizedReview> {
    let text = &job.settings.onscreen_text;
    if !(text.enabled && text.localized_video) {
        return None;
    }
    let video = work_dir::read_json::<LocalizedVideoRecord>(&work.text(StepName::LocalizedVideo))
        .ok()
        .and_then(|record| record.path)
        .map(PathBuf::from)
        .filter(|path| path.is_file());
    let subtitles = output
        .localized
        .as_ref()
        .map(PathBuf::from)
        .filter(|path| path.is_file());
    let replacements = checked_replacements(work)
        .map(|verified| replacements(work.root(), &verified, document))
        .unwrap_or_default();
    Some(LocalizedReview {
        mode: default_mode(),
        video,
        subtitles,
        replacements,
        show_mask: false,
        pictures: None,
    })
}

/// The picture a job with a localized video shows on the right at first: the localized video,
/// or its replaced plates until it is written.
pub(crate) fn default_mode() -> PreviewMode {
    PreviewMode::Localized
}

/// The replacements as the read-back check left them, else, in a job from before the check, as
/// composition left them, without checks.
pub(crate) fn checked_replacements(work: &WorkDir) -> Option<VerifiedReplacements> {
    work_dir::read_json::<VerifiedReplacements>(&work.text(StepName::TextVerify))
        .ok()
        .or_else(|| {
            work_dir::read_json::<ReplacementDocument>(&work.text(StepName::TextCompose))
                .ok()
                .map(|document| VerifiedReplacements {
                    document,
                    checks: Vec::new(),
                })
        })
}

/// Each occurrence's replacement by id, with its keyframe plate's erase mask and what the
/// read-back check read.
pub(crate) fn replacements(
    root: &Path,
    verified: &VerifiedReplacements,
    document: &TextDocument,
) -> BTreeMap<String, Replacement> {
    verified
        .document
        .texts
        .iter()
        .map(|text| {
            let occurrence = document.occurrences.iter().find(|o| o.id == text.id);
            let plate = keyframe_plate(&text.plates, keyframe_frame(text, occurrence));
            let mask = plate.and_then(|plate| {
                Some(MaskPlate {
                    rect: plate.rect,
                    path: inside(root, &plate.mask)?,
                })
            });
            let replacement = Replacement {
                status: text.status.clone(),
                preview: text.preview.as_deref().and_then(|path| inside(root, path)),
                mask,
                check: verified
                    .check(&text.id)
                    .and_then(TextCheck::telling)
                    .map(check_line),
            };
            (text.id.clone(), replacement)
        })
        .collect()
}

/// The frame of `text`'s keyframe: its keyframe time's share of the occurrence's span, applied
/// to the text's frames; the middle of the span without a keyframe, its first frame without an
/// occurrence or a span.
pub(crate) fn keyframe_frame(text: &ReplacedText, occurrence: Option<&TextOccurrence>) -> u64 {
    let Some(occurrence) = occurrence else {
        return text.first_frame;
    };
    let span_s = occurrence.end_s - occurrence.start_s;
    let frames = text.last_frame.saturating_sub(text.first_frame) + 1;
    if !(span_s.is_finite() && span_s > 0.0) {
        return text.first_frame;
    }
    let time = occurrence
        .keyframe
        .as_ref()
        .map_or((occurrence.start_s + occurrence.end_s) / 2.0, |key| {
            key.time_s
        });
    let offset = ((time - occurrence.start_s) / span_s * frames as f64).floor();
    let offset = if offset.is_finite() {
        offset.max(0.0) as u64
    } else {
        0
    };
    text.first_frame + offset.min(frames - 1)
}

/// The plate covering `frame`, else the one nearest to it.
pub(crate) fn keyframe_plate(plates: &[Plate], frame: u64) -> Option<&Plate> {
    plates.iter().min_by_key(|plate| {
        if frame < plate.first_frame {
            plate.first_frame - frame
        } else {
            frame.saturating_sub(plate.last_frame)
        }
    })
}

/// Where `rect` sits in a `width` × `height` frame, as shares of its width and height: left, top,
/// right, bottom, each from 0 to 1; none for an empty frame or rectangle.
pub(crate) fn mask_area(rect: PixelRect, width: u32, height: u32) -> Option<[f32; 4]> {
    if width == 0 || height == 0 || rect.width == 0 || rect.height == 0 {
        return None;
    }
    let (w, h) = (width as f32, height as f32);
    let share = |value: u32, of: f32| (value as f32 / of).clamp(0.0, 1.0);
    Some([
        share(rect.x, w),
        share(rect.y, h),
        share(rect.right(), w),
        share(rect.bottom(), h),
    ])
}

/// What the preview says about the selected occurrence: replaced, or not with the reason.
pub(crate) fn status_line(replacement: Option<&Replacement>) -> String {
    match replacement.map(|replacement| &replacement.status) {
        Some(ReplaceStatus::Baked) => "Replaced in the video".into(),
        Some(ReplaceStatus::Fallback(reason)) => format!("Not replaced in the video: {reason}"),
        Some(ReplaceStatus::Pending) | None => {
            "Not replaced in the video: its replacement has not been made yet".into()
        }
    }
}

/// What the read-back check read from the frame that says most about a replacement.
pub(crate) fn check_line(reading: &VerifyReading) -> String {
    if !reading.japanese_found.is_empty() {
        format!("Checked: Japanese still reads “{}”", reading.japanese_found)
    } else if reading.english_read.is_empty() {
        "Checked: no English reads back".into()
    } else {
        format!("Checked: English reads back as “{}”", reading.english_read)
    }
}

/// Decode `replacement`'s replaced plate and erase mask for occurrence `id`.
pub(crate) fn pictures(id: &str, replacement: &Replacement) -> ReplacementPictures {
    let preview = replacement.preview.as_deref().and_then(|path| {
        let image = decode(path)?.to_rgb8();
        Some(player::picture(
            image.width(),
            image.height(),
            image.into_raw(),
        ))
    });
    let mask = replacement.mask.as_ref().and_then(|plate| {
        let image = decode(&plate.path)?.to_luma8();
        Some(Mask {
            serial: player::serial(),
            width: image.width(),
            height: image.height(),
            coverage: image.into_raw(),
            rect: plate.rect,
        })
    });
    ReplacementPictures {
        id: id.to_string(),
        preview,
        mask,
    }
}

/// `relative` under the job folder `root`, when it names a path inside it.
fn inside(root: &Path, relative: &Path) -> Option<PathBuf> {
    relative
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
        .then(|| root.join(relative))
}

/// A PNG decoded under allocation limits and shrunk to at most `PICTURE_EDGE` on a side.
fn decode(path: &Path) -> Option<image::DynamicImage> {
    let mut reader = image::ImageReader::open(path).ok()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    reader.limits(limits);
    let image = reader.decode().ok()?;
    if image.width().max(image.height()) > PICTURE_EDGE {
        Some(image.thumbnail(PICTURE_EDGE, PICTURE_EDGE))
    } else {
        Some(image)
    }
}

#[cfg(test)]
#[path = "tests/localized.rs"]
mod tests;
