//! The read-back check: a local OCR reads each lettered replacement off the finished picture,
//! and only replacements that read back cleanly stay in the localized video.
//!
//! **Role:** for every baked occurrence, rebuild a few sampled frames as the localized video
//! shows them (the source region with every active patch blended in), find and read the lines
//! over the lettering, and keep the occurrence baked only when no frame still shows Japanese and
//! every frame reads back as its English.
//! **Position:** the `text_verify` step, after composition and before the localized video; the
//! pipeline runs it in an ONNX Runtime worker with `LocalOcr` (PP-OCRv5 on CUDA). `samples`
//! picks the frames, `area` the region and the lines that belong to the lettering, `verdict`
//! the judgement; the composite is `localize::still`, over `localize::patches::Schedule`.
//! **Signals and state:** one decoded region and its patches at a time; the composed document is
//! only read, and the verified copy it returns carries the final statuses and every reading.
//! **Invariants:** only baked occurrences are checked and only a check's failure changes one, to
//! `Fallback` with `JAPANESE_LEFT` or `UNREADABLE`; the patches blended into a sample are those
//! the localized video blends at that frame, in the same order; files are only read.

pub mod area;
pub mod samples;
pub mod verdict;

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use image::{RgbImage, RgbaImage, imageops};
use inference::ocr::{OcrDetector, OcrReader, TextDetection};
use job_model::onscreen::{
    PixelRect, Quad, ReplaceStatus, ReplacementDocument, TextCheck, TextDocument, TextOccurrence,
    VerifiedReplacements, VerifyReading,
};

use self::area::SampleArea;
use self::samples::Candidate;
pub use self::verdict::ReadLine;
use super::RegionSource;
use crate::localize::colour::Conversion;
use crate::localize::patches::Schedule;
use crate::localize::still::{PlacedPatch, finished_region};
use crate::onscreen_text::TextResult;
use crate::onscreen_text::detect::crop;

/// Finds and reads lines of writing in a picture.
pub trait ReadBack {
    /// Every line found, with its box score.
    fn find(&mut self, image: &RgbImage) -> TextResult<Vec<(Quad, f64)>>;
    /// One upright line's text and confidence.
    fn read(&mut self, image: &RgbImage) -> TextResult<(String, f64)>;
}

/// PP-OCRv5's server detector and its recognizer alone, which reads Japanese and Latin alike.
pub struct LocalOcr {
    detector: OcrDetector,
    reader: OcrReader,
}

impl LocalOcr {
    /// Open both models under `models`; must run first in an ONNX Runtime worker.
    pub fn open(models: &Path) -> TextResult<Self> {
        Ok(Self {
            detector: OcrDetector::open(models)?,
            reader: OcrReader::open(models)?,
        })
    }
}

impl ReadBack for LocalOcr {
    fn find(&mut self, image: &RgbImage) -> TextResult<Vec<(Quad, f64)>> {
        self.detector.detect(image)
    }

    fn read(&mut self, image: &RgbImage) -> TextResult<(String, f64)> {
        self.reader.read_primary(image)
    }
}

/// What the check reads from.
pub struct Request<'a> {
    /// The composed replacements; patch paths are relative to `root`.
    pub composed: &'a ReplacementDocument,
    /// The reviewed occurrences, with their English.
    pub text: &'a TextDocument,
    /// The job directory.
    pub root: &'a Path,
    /// The source's colour conversion, as the localized video blends in it.
    pub conversion: Conversion,
    /// Occurrence ids to check; every baked occurrence when `None`.
    pub only: Option<&'a [String]>,
}

/// One line found in a sample.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundLine {
    /// Where it is in the upscaled region.
    pub quad: Quad,
    pub score: f64,
    /// What it reads and where it counts.
    pub read: ReadLine,
}

/// One sampled frame as the check saw it.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub id: String,
    pub area: SampleArea,
    pub lines: Vec<FoundLine>,
    pub reading: VerifyReading,
}

/// Where the check shows each sample and the upscaled finished region it read.
pub type Observe<'a> = &'a mut dyn FnMut(&Sample, &RgbImage);

/// Check every baked occurrence of the request; `progress` hears `(samples done, samples)`.
pub fn verify(
    request: &Request,
    source: &mut dyn RegionSource,
    reader: &mut dyn ReadBack,
    observe: Observe,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> TextResult<VerifiedReplacements> {
    let composed = request.composed;
    let occurrences: HashMap<&str, &TextOccurrence> = request
        .text
        .occurrences
        .iter()
        .map(|occurrence| (occurrence.id.as_str(), occurrence))
        .collect();
    let checked: Vec<(usize, &TextOccurrence, Candidate)> = composed
        .texts
        .iter()
        .enumerate()
        .filter(|(_, text)| text.status == ReplaceStatus::Baked)
        .filter(|(_, text)| request.only.is_none_or(|ids| ids.contains(&text.id)))
        .filter_map(|(index, text)| {
            let occurrence = *occurrences.get(text.id.as_str())?;
            let quad = area::keyframe_quad(text, occurrence)?;
            let candidate = Candidate {
                first_frame: text.first_frame,
                last_frame: text.last_frame,
                plate_starts: text.plates.iter().map(|plate| plate.first_frame).collect(),
                area: quad.bounds(),
            };
            Some((index, occurrence, candidate))
        })
        .collect();
    let mut by_frame: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (position, (_, _, candidate)) in checked.iter().enumerate() {
        let others: Vec<&Candidate> = checked
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != position)
            .map(|(_, (_, _, other))| other)
            .collect();
        for frame in samples::sample_frames(candidate, &others) {
            by_frame.entry(frame).or_default().push(position);
        }
    }
    let total = by_frame.values().map(Vec::len).sum();
    progress(0, total);
    let size = (composed.width, composed.height);
    let mut schedule = Schedule::new(composed);
    let mut readings: Vec<Vec<VerifyReading>> = vec![Vec::new(); checked.len()];
    let mut done = 0;
    for (&frame, positions) in &by_frame {
        schedule.advance(frame)?;
        for &position in positions {
            let (index, occurrence, _) = checked[position];
            let text = &composed.texts[index];
            if let Some(area) = area::sample_area(text, occurrence, frame, size) {
                let english = occurrence.english.as_deref().unwrap_or_default();
                let finished = finished_picture(request, source, &schedule, frame, &area)?;
                let sample = read_sample(reader, &text.id, frame, area, &finished, english)?;
                observe(&sample, &finished);
                readings[position].push(sample.reading);
            }
            done += 1;
            progress(done, total);
        }
    }
    let mut verified = VerifiedReplacements {
        document: composed.clone(),
        checks: Vec::with_capacity(checked.len()),
    };
    for ((index, _, _), readings) in checked.iter().zip(readings) {
        let text = &mut verified.document.texts[*index];
        if let Some(reason) = verdict::verdict(&readings) {
            text.status = ReplaceStatus::Fallback(reason.to_string());
        }
        verified.checks.push(TextCheck {
            id: text.id.clone(),
            readings,
        });
    }
    Ok(verified)
}

/// The region of `area` at `frame` as the localized video shows it, enlarged by its scale.
fn finished_picture(
    request: &Request,
    source: &mut dyn RegionSource,
    schedule: &Schedule,
    frame: u64,
    area: &SampleArea,
) -> TextResult<RgbImage> {
    let region = area.region;
    let mut pixels = None;
    source.frames(region, frame, frame, &mut |_, image| {
        pixels = Some(image);
        Ok(())
    })?;
    let pixels = pixels.ok_or_else(|| format!("frame {frame} could not be decoded"))?;
    let mut images: Vec<(RgbaImage, PixelRect)> = Vec::new();
    for patch in schedule.active() {
        if patch.rect.overlaps(region) {
            let path = request.root.join(&patch.path);
            let image = image::open(&path)
                .map_err(|e| format!("reading {}: {e}", path.display()))?
                .to_rgba8();
            images.push((image, patch.rect));
        }
    }
    let placed: Vec<PlacedPatch> = images
        .iter()
        .map(|(image, rect)| PlacedPatch { image, rect: *rect })
        .collect();
    let finished = finished_region(&pixels, region, &placed, request.conversion)?;
    if area.scale <= 1.0 {
        return Ok(finished);
    }
    let width = (f64::from(region.width) * area.scale).round().max(1.0) as u32;
    let height = (f64::from(region.height) * area.scale).round().max(1.0) as u32;
    Ok(imageops::resize(
        &finished,
        width,
        height,
        imageops::FilterType::CatmullRom,
    ))
}

/// Find the lines of `picture`, read each in reading order, and judge the frame.
fn read_sample(
    reader: &mut dyn ReadBack,
    id: &str,
    frame: u64,
    area: SampleArea,
    picture: &RgbImage,
    english: &str,
) -> TextResult<Sample> {
    let mut found = reader.find(picture)?;
    area::reading_order(&mut found);
    let mut lines = Vec::with_capacity(found.len());
    for (quad, score) in found {
        let (text, confidence) = reader.read(&crop(picture, quad))?;
        lines.push(FoundLine {
            quad,
            score,
            read: ReadLine {
                text,
                confidence,
                over_lettering: area::over_lettering(&area, quad),
                over_writing: area::over_writing(&area, quad),
            },
        });
    }
    let read: Vec<ReadLine> = lines.iter().map(|line| line.read.clone()).collect();
    Ok(Sample {
        id: id.to_string(),
        reading: verdict::judge(frame, &read, english),
        area,
        lines,
    })
}

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixtures;

#[cfg(test)]
#[path = "tests/verify.rs"]
mod tests;
