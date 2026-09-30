//! Local readings of detected text crops.
//!
//! **Role:** retain readable Japanese and explicitly unresolved observations.
//! **Position:** second visual stage, between detection and tracking.
//! **Signals and state:** crop files and one local OCR reader.
//! **Invariants:** a confident non-Japanese reading is excluded; uncertain readings stay flagged.

use super::{TextResult, furigana};
use inference::ocr::OcrReader;
use job_model::onscreen::{Quad, TextCorrections, TextDocument, TextOccurrence, TextTreatment};
use job_model::outputs::ShotChanges;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    path::Path,
};

const READING_CACHE_REVISION: u32 = 2;
/// Extra fraction of the shortest frame allowed between adjacent readings, so millisecond-rounded
/// timestamps of a one-frame gap still count as one frame.
const HALF_FRAME_SLACK: f64 = 0.5;

#[derive(Serialize, Deserialize)]
struct CachedReading {
    revision: u32,
    text: String,
    confidence: f64,
}

pub fn read(
    document: &mut TextDocument,
    root: &Path,
    reader: &mut OcrReader,
    corrections: &TextCorrections,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> TextResult<()> {
    let total = document.occurrences.len();
    let cache = root.join("visual/readings");
    std::fs::create_dir_all(&cache)?;
    for (index, item) in document.occurrences.iter_mut().enumerate() {
        let path = item
            .crops
            .first()
            .ok_or("detected text has no representative crop")?;
        let bytes = std::fs::read(root.join(path))?;
        let generation = corrections
            .retry
            .iter()
            .filter(|id| **id == item.id)
            .count();
        let file = cache.join(format!(
            "{generation}-{}",
            reading_key(&bytes, READING_CACHE_REVISION)
        ));
        let cached = std::fs::read(&file)
            .ok()
            .and_then(|bytes| cached_reading(&bytes));
        let (text, confidence) = match cached {
            Some(answer) => answer,
            None => {
                let crop = image::load_from_memory(&bytes)?.to_rgb8();
                let answer = reader.read(&crop)?;
                if !valid_confidence(answer.1) {
                    return Err("OCR reader returned invalid confidence".into());
                }
                let record = CachedReading {
                    revision: READING_CACHE_REVISION,
                    text: answer.0.clone(),
                    confidence: answer.1,
                };
                let part = file.with_extension("part");
                std::fs::write(&part, serde_json::to_vec(&record)?)?;
                std::fs::rename(part, &file)?;
                answer
            }
        };
        item.japanese = text;
        item.confidence = confidence;
        item.provenance.backend = "PP-OCRv5 / manga-ocr".into();
        if confidence < 0.88 || item.japanese.trim().is_empty() {
            item.warnings
                .push("The local readers could not agree on a confident reading.".into());
        }
        progress(index + 1, total);
    }
    document.occurrences.retain(|item| {
        japanese(&item.japanese) || item.confidence < 0.88 || item.japanese.is_empty()
    });
    let cuts = load_cuts(root)?;
    consolidate_readings(document, &cuts);
    furigana::group_furigana(document);
    Ok(())
}

pub(super) fn load_cuts(root: &Path) -> TextResult<ShotChanges> {
    match std::fs::read(root.join("shots.json")) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ShotChanges::default()),
        Err(error) => Err(error.into()),
    }
}

fn reading_key(bytes: &[u8], revision: u32) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    revision.hash(&mut hash);
    bytes.hash(&mut hash);
    for file in inference::model_store::MODEL_FILES
        .iter()
        .filter(|file| matches!(file.model, "pp-ocrv5" | "manga-ocr"))
    {
        file.sha256.hash(&mut hash);
    }
    format!("{:016x}.json", hash.finish())
}

fn valid_confidence(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn cached_reading(bytes: &[u8]) -> Option<(String, f64)> {
    let answer: CachedReading = serde_json::from_slice(bytes).ok()?;
    (answer.revision == READING_CACHE_REVISION && valid_confidence(answer.confidence))
        .then_some((answer.text, answer.confidence))
}

/// Consolidation identifies an occurrence, not verified tracking; all observed quads survive.
pub(super) fn consolidate_readings(document: &mut TextDocument, cuts: &ShotChanges) {
    let mut items = std::mem::take(&mut document.occurrences);
    items.sort_by(|a, b| a.start_s.total_cmp(&b.start_s).then(a.id.cmp(&b.id)));
    let frame_s = items
        .iter()
        .flat_map(|item| &item.frames)
        .map(|frame| frame.end_s - frame.time_s)
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .reduce(f64::min)
        .unwrap_or(0.0);
    let max_gap_s = frame_s * (1.0 + HALF_FRAME_SLACK) + 1e-6;
    let mut active: HashMap<String, Vec<usize>> = HashMap::new();
    let mut valid = Vec::new();
    let mut remaining = items.into_iter();
    while let Some(item) = remaining.next() {
        let item_valid = observed_bounds(&item);
        let candidates = active.entry(item.japanese.clone()).or_default();
        candidates.retain(|&index| document.occurrences[index].end_s + max_gap_s >= item.start_s);
        let mut matches = candidates.iter().copied().filter(|&index| {
            valid[index]
                && item_valid
                && adjacent_reading(&document.occurrences[index], &item, max_gap_s, cuts)
                && !remaining
                    .as_slice()
                    .iter()
                    .take_while(|other| {
                        other.start_s <= document.occurrences[index].end_s + max_gap_s
                    })
                    .any(|other| {
                        other.start_s < item.end_s
                            && other.end_s > item.start_s
                            && adjacent_reading(
                                &document.occurrences[index],
                                other,
                                max_gap_s,
                                cuts,
                            )
                    })
                && !document
                    .occurrences
                    .iter()
                    .rev()
                    .take_while(|other| other.start_s >= document.occurrences[index].end_s - 1e-6)
                    .any(|other| {
                        (other.japanese != item.japanese || other.english != item.english)
                            && other.start_s < item.start_s
                            && other
                                .frames
                                .first()
                                .zip(item.frames.first())
                                .is_some_and(|(a, b)| matching_region(a.quad, b.quad))
                    })
        });
        let index = matches.next();
        let unique = matches.next().is_none();
        if let Some(index) = index.filter(|_| unique) {
            let base = &mut document.occurrences[index];
            append_reason(
                base,
                &format!(
                    "Adjacent OCR evidence {} ({:.2} confidence; {}): {}; original frame geometry retained.",
                    item.id, item.confidence, item.provenance.backend, item.provenance.reason
                ),
            );
            base.end_s = item.end_s;
            base.confidence = base.confidence.min(item.confidence);
            if base.presentation.treatment != item.presentation.treatment {
                base.presentation.treatment = TextTreatment::Nearby;
            }
            base.source_fingerprint = None;
            // The frame missing between two sightings shows the same writing: no hole remains.
            if let Some(last) = base.frames.last_mut() {
                last.end_s = last.end_s.max(item.start_s);
            }
            base.frames.extend(item.frames);
            for crop in item.crops {
                if !base.crops.contains(&crop) {
                    base.crops.push(crop);
                }
            }
            for warning in item.warnings {
                if !base.warnings.contains(&warning) {
                    base.warnings.push(warning);
                }
            }
        } else {
            candidates.push(document.occurrences.len());
            document.occurrences.push(item);
            valid.push(item_valid);
        }
    }
}

fn adjacent_reading(
    a: &TextOccurrence,
    b: &TextOccurrence,
    max_gap_s: f64,
    cuts: &ShotChanges,
) -> bool {
    let (Some(last), Some(first)) = (a.frames.last(), b.frames.first()) else {
        return false;
    };
    let gap = b.start_s - a.end_s;
    japanese(&a.japanese)
        && a.japanese == b.japanese
        && a.english == b.english
        && a.presentation.anchor == b.presentation.anchor
        && a.presentation.font_size == b.presentation.font_size
        && !a.reviewed
        && !b.reviewed
        && a.provenance.reference == b.provenance.reference
        && valid_confidence(a.confidence)
        && valid_confidence(b.confidence)
        && (a.english.is_none() || (a.confidence >= 0.85) == (b.confidence >= 0.85))
        && gap >= -1e-6
        && gap <= max_gap_s
        && (a.end_s - last.end_s).abs() <= 1e-6
        && (b.start_s - first.time_s).abs() <= 1e-6
        && !cuts
            .cuts
            .iter()
            .any(|cut| cut.time_s > last.time_s && cut.time_s <= first.time_s + 1e-6)
        && matching_region(last.quad, first.quad)
}

fn matching_region(a: Quad, b: Quad) -> bool {
    if !a.valid() || !b.valid() {
        return false;
    }
    let (al, at, ar, ab) = a.bounds();
    let (bl, bt, br, bb) = b.bounds();
    let intersection = (ar.min(br) - al.max(bl)).max(0.0) * (ab.min(bb) - at.max(bt)).max(0.0);
    let union = (ar - al) * (ab - at) + (br - bl) * (bb - bt) - intersection;
    let side = (ar - al).min(ab - at).min(br - bl).min(bb - bt);
    intersection / union >= 0.6
        && a.0
            .iter()
            .zip(b.0)
            .all(|(a, b)| (a.x - b.x).hypot(a.y - b.y) <= side * 0.45)
}

pub(super) fn append_reason(item: &mut TextOccurrence, reason: &str) {
    if !item.provenance.reason.is_empty() {
        item.provenance.reason.push(' ');
    }
    item.provenance.reason.push_str(reason);
}

pub(super) fn observed_bounds(item: &TextOccurrence) -> bool {
    item.start_s.is_finite()
        && item.end_s.is_finite()
        && item.end_s > item.start_s
        && item
            .frames
            .first()
            .is_some_and(|frame| (frame.time_s - item.start_s).abs() <= 1e-6)
        && item
            .frames
            .last()
            .is_some_and(|frame| (frame.end_s - item.end_s).abs() <= 1e-6)
        && item.frames.iter().all(|frame| {
            frame.time_s.is_finite()
                && frame.end_s.is_finite()
                && frame.end_s > frame.time_s
                && frame.quad.valid()
        })
        && item
            .frames
            .windows(2)
            .all(|pair| pair[1].time_s >= pair[0].end_s - 1e-6)
}

pub fn japanese(text: &str) -> bool {
    text.chars().any(|c| matches!(c, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{9fff}' | '\u{ff66}'..='\u{ff9f}'))
}

#[cfg(test)]
#[path = "tests/read.rs"]
mod tests;
