//! Whole-keyframe Claude requests: grouping, prompts, the answer schema and applying answers.
//!
//! **Role:** group occurrences by the keyframe still they were seen on, describe each region to
//! Claude, check the structured answer, and apply it, adding writing the local detector missed.
//! **Position:** private helper of the visual translation stage; `vision` sends the requests.
//! **Signals and state:** owned request values; answers change the document only in [`apply`].
//! **Invariants:** a request lists at most 64 regions; an answer applies only when it answers
//! every listed region exactly once with valid values; writing Claude adds is flagged for review
//! and never replaces an observed occurrence.

use super::{
    SYSTEM, TextResult, TranslationInput, append_translation_reason, nearby_dialogue, read_cached,
    retry_generation, valid_text, write_cached,
};
use job_model::onscreen::{
    Point, Quad, TextDocument, TextFrame, TextOccurrence, TextPresentation, TextProvenance,
    TextTreatment,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const KEYFRAME_INSTRUCTIONS: &str = "Image 1 is the whole frame: inspect it independently before using any OCR reading, which may be wrong or describe non-text artwork. Read each listed region from its crop and confirm it on the frame, answering every listed region exactly once under its id. Every bbox is a normalised whole-frame box [x0, y0, x1, y1] with 0,0 at the top left and 1,1 at the bottom right; give where the writing actually is. Put visible Japanese writing that no listed region covers into other_text with its own bbox. For a region with no readable writing, return an empty Japanese string, null English and a reason. Never complete missing or obscured characters from context. English holds only the translation.";
/// The most regions one request lists; a larger group is left to the local model.
const MAX_REGIONS: usize = 64;
/// The most unlisted writings one answer adds.
const MAX_OTHER_TEXT: usize = 16;
/// Below this overlap with the detector's box, Claude read the writing somewhere else.
const MIN_BOX_OVERLAP: f64 = 0.3;
/// From this overlap with a listed region, unlisted writing repeats that region instead of
/// adding one.
const DUPLICATE_BOX_OVERLAP: f64 = 0.5;
pub(super) const INVALID_RESPONSE: &str = "Claude returned an invalid visual response.";
const LOCATED_ELSEWHERE: &str = "Claude located the writing elsewhere; review it.";
const FOUND_BY_CLAUDE: &str = "Found by Claude on the keyframe; the local detector did not see it; timing follows the surrounding event.";

/// One occurrence as a request lists it.
#[derive(Debug)]
pub(super) struct Region {
    pub index: usize,
    pub id: String,
    /// The first crop, joined to the job root.
    pub crop: PathBuf,
    /// The occurrence's box on the keyframe: normalised `[x0, y0, x1, y1]`, 0,0 top left.
    pub bbox: [f64; 4],
    pub ocr_reading: String,
    pub reading_confidence: f64,
}

/// Every occurrence seen on one keyframe still, asked about in one Claude call.
#[derive(Debug)]
pub(super) struct Request {
    /// The whole-frame still, joined to the job root.
    pub keyframe: PathBuf,
    pub regions: Vec<Region>,
    pub prompt: String,
    /// The highest retry generation among the regions.
    pub retry_generation: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct KeyframeAnswer {
    pub regions: Vec<RegionAnswer>,
    pub other_text: Vec<FoundText>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RegionAnswer {
    pub id: String,
    pub japanese: String,
    pub english: Option<String>,
    pub confidence: f64,
    pub reason: String,
    pub bbox: [f64; 4],
}

/// Writing on the frame that no listed region covers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FoundText {
    pub japanese: String,
    pub english: Option<String>,
    pub confidence: f64,
    pub reason: String,
    pub bbox: [f64; 4],
}

/// What one request produced.
#[derive(Debug)]
pub(super) enum Outcome {
    /// A checked answer whose regions follow the request's order.
    Answer(KeyframeAnswer),
    /// Why the request's occurrences go to the local model.
    Warning(String),
}

#[derive(Serialize)]
struct Prompt<'a> {
    images: String,
    regions: Vec<PromptRegion<'a>>,
    nearby_dialogue: String,
    glossary: &'a [String],
}

#[derive(Serialize)]
struct PromptRegion<'a> {
    id: String,
    image: usize,
    bbox: [f64; 4],
    ocr_reading: &'a str,
    reading_confidence: f64,
}

/// One request per keyframe still, in order of first appearance. Occurrences without a keyframe,
/// a crop or a usable box on the keyframe are not requested.
pub(super) fn build(
    document: &TextDocument,
    input: &TranslationInput<'_>,
) -> TextResult<Vec<Request>> {
    let mut groups: Vec<(&Path, Vec<Region>)> = Vec::new();
    let mut positions = HashMap::<&Path, usize>::new();
    for (index, item) in document.occurrences.iter().enumerate() {
        let (Some(keyframe), Some(crop), Some(bbox)) =
            (&item.keyframe, item.crops.first(), hint(document, item))
        else {
            continue;
        };
        let position = *positions
            .entry(keyframe.image.as_path())
            .or_insert(groups.len());
        if position == groups.len() {
            groups.push((keyframe.image.as_path(), Vec::new()));
        }
        groups[position].1.push(Region {
            index,
            id: item.id.clone(),
            crop: input.root.join(crop),
            bbox,
            ocr_reading: item.japanese.clone(),
            reading_confidence: item.confidence,
        });
    }
    let mut requests = Vec::with_capacity(groups.len());
    for (image, regions) in groups {
        if regions.len() > MAX_REGIONS {
            tracing::warn!(
                keyframe = %image.display(),
                regions = regions.len(),
                "Too many regions for one Claude keyframe request; the local model translates them"
            );
            continue;
        }
        let earliest = regions
            .iter()
            .map(|region| document.occurrences[region.index].start_s)
            .fold(f64::INFINITY, f64::min);
        let generation = regions
            .iter()
            .map(|region| retry_generation(input.corrections, &region.id))
            .max()
            .unwrap_or(0);
        let prompt = serde_json::to_string(&Prompt {
            images: images_note(regions.len()),
            regions: regions
                .iter()
                .enumerate()
                .map(|(position, region)| PromptRegion {
                    id: label(position),
                    image: position + 2,
                    bbox: region.bbox,
                    ocr_reading: &region.ocr_reading,
                    reading_confidence: rounded(region.reading_confidence, 1000.0),
                })
                .collect(),
            nearby_dialogue: nearby_dialogue(input.dialogue, earliest),
            glossary: input.glossary,
        })?;
        requests.push(Request {
            keyframe: input.root.join(image),
            regions,
            prompt,
            retry_generation: generation,
        });
    }
    Ok(requests)
}

pub(super) fn system() -> String {
    format!("{SYSTEM} {KEYFRAME_INSTRUCTIONS}")
}

/// The answer schema: closed objects with every field required.
pub(super) fn schema() -> serde_json::Value {
    let text = |with_id: bool| {
        let mut properties = json!({
            "japanese": {"type": "string", "maxLength": 500},
            "english": {"type": ["string", "null"], "maxLength": 500},
            "confidence": {"type": "number", "minimum": 0, "maximum": 1},
            "reason": {"type": "string", "maxLength": 240},
            "bbox": {"type": "array", "minItems": 4, "maxItems": 4,
                "items": {"type": "number", "minimum": 0, "maximum": 1}},
        });
        let mut required = vec!["japanese", "english", "confidence", "reason", "bbox"];
        if with_id {
            properties["id"] = json!({"type": "string", "maxLength": 8});
            required.insert(0, "id");
        }
        json!({"type": "object", "additionalProperties": false,
            "properties": properties, "required": required})
    };
    json!({"type": "object", "additionalProperties": false, "properties": {
        "regions": {"type": "array", "maxItems": MAX_REGIONS, "items": text(true)},
        "other_text": {"type": "array", "maxItems": MAX_OTHER_TEXT, "items": text(false)},
    }, "required": ["regions", "other_text"]})
}

/// The answer with its regions in request order, when it answers every listed region exactly
/// once with valid values.
pub(super) fn checked(request: &Request, mut answer: KeyframeAnswer) -> Option<KeyframeAnswer> {
    if answer.regions.len() != request.regions.len() {
        return None;
    }
    let mut ordered = Vec::with_capacity(answer.regions.len());
    for position in 0..request.regions.len() {
        let label = label(position);
        let found = answer
            .regions
            .iter()
            .position(|region| region.id == label)?;
        ordered.push(answer.regions.swap_remove(found));
    }
    let valid = ordered.iter().all(|region| {
        valid_text(
            &region.japanese,
            region.english.as_deref(),
            region.confidence,
            &region.reason,
        ) && valid_box(region.bbox)
    });
    answer.regions = ordered;
    valid.then_some(answer)
}

pub(super) fn read_cache(path: &Path, request: &Request) -> Option<KeyframeAnswer> {
    checked(request, read_cached(path)?)
}

pub(super) fn write_cache(path: &Path, answer: &KeyframeAnswer) -> TextResult<()> {
    write_cached(path, answer)
}

/// Apply one request's outcome. Answered occurrences get a reason; added writing is appended
/// with its reason, so `reasons` keeps one entry per occurrence.
pub(super) fn apply(
    document: &mut TextDocument,
    request: &Request,
    outcome: Outcome,
    backend: &str,
    reasons: &mut Vec<Option<String>>,
) {
    let answer = match outcome {
        Outcome::Answer(answer) => answer,
        Outcome::Warning(warning) => {
            for region in &request.regions {
                document.occurrences[region.index]
                    .warnings
                    .push(warning.clone());
            }
            return;
        }
    };
    for (region, found) in request.regions.iter().zip(&answer.regions) {
        let item = &mut document.occurrences[region.index];
        item.japanese.clone_from(&found.japanese);
        item.confidence = found.confidence;
        item.english = translation(found.english.as_deref());
        item.provenance.backend = backend.into();
        append_translation_reason(item, &found.reason);
        if intersection_over_union(found.bbox, region.bbox) < MIN_BOX_OVERLAP {
            item.warnings.push(LOCATED_ELSEWHERE.into());
        }
        reasons[region.index] = Some(found.reason.clone());
    }
    add_other_text(document, request, &answer.other_text, backend, reasons);
}

/// Unlisted writing becomes a nearby label spanning the group's event, flagged for review.
fn add_other_text(
    document: &mut TextDocument,
    request: &Request,
    found: &[FoundText],
    backend: &str,
    reasons: &mut Vec<Option<String>>,
) {
    let Some(first) = request.regions.first() else {
        return;
    };
    let base = &document.occurrences[first.index];
    let (id, Some(keyframe)) = (base.id.clone(), base.keyframe.clone()) else {
        return;
    };
    let members = || {
        request
            .regions
            .iter()
            .map(|region| &document.occurrences[region.index])
    };
    let start_s = members()
        .map(|item| item.start_s)
        .fold(f64::INFINITY, f64::min);
    let end_s = members()
        .map(|item| item.end_s)
        .fold(f64::NEG_INFINITY, f64::max);
    let (width, height) = (f64::from(document.width), f64::from(document.height));
    for (position, text) in found.iter().enumerate().take(MAX_OTHER_TEXT) {
        let english = text.english.as_deref();
        if text.japanese.trim().is_empty()
            || !valid_box(text.bbox)
            || !valid_text(&text.japanese, english, text.confidence, &text.reason)
            || request.regions.iter().any(|region| {
                intersection_over_union(text.bbox, region.bbox) >= DUPLICATE_BOX_OVERLAP
            })
        {
            continue;
        }
        let [x0, y0, x1, y1] = text.bbox;
        let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        let mut item = TextOccurrence {
            id: format!("{id}-c{}", position + 1),
            start_s,
            end_s,
            japanese: text.japanese.clone(),
            english: translation(english),
            confidence: text.confidence,
            crops: vec![keyframe.image.clone()],
            frames: vec![TextFrame {
                time_s: start_s,
                end_s,
                quad: Quad(corners.map(|(x, y)| Point {
                    x: x * width,
                    y: y * height,
                })),
                confidence: text.confidence,
                surface_rgb: None,
            }],
            provenance: TextProvenance {
                backend: backend.into(),
                ..TextProvenance::default()
            },
            presentation: TextPresentation {
                treatment: TextTreatment::Nearby,
                ..TextPresentation::default()
            },
            warnings: vec![FOUND_BY_CLAUDE.into()],
            reviewed: false,
            rendered: None,
            source_fingerprint: None,
            keyframe: Some(keyframe.clone()),
        };
        append_translation_reason(&mut item, &text.reason);
        document.occurrences.push(item);
        reasons.push(Some(text.reason.clone()));
    }
}

/// The occurrence's box on its keyframe, normalised to the whole frame: from the frame observed
/// at the keyframe's time, else from its first frame.
fn hint(document: &TextDocument, item: &TextOccurrence) -> Option<[f64; 4]> {
    let keyframe = item.keyframe.as_ref()?;
    let frame = item
        .frames
        .iter()
        .find(|frame| (frame.time_s - keyframe.time_s).abs() <= 1e-6)
        .or(item.frames.first())?;
    if !frame.quad.valid() || document.width == 0 || document.height == 0 {
        return None;
    }
    let (left, top, right, bottom) = frame.quad.bounds();
    let (width, height) = (f64::from(document.width), f64::from(document.height));
    let bbox = [left / width, top / height, right / width, bottom / height]
        .map(|value| rounded(value.clamp(0.0, 1.0), 10_000.0));
    valid_box(bbox).then_some(bbox)
}

fn images_note(regions: usize) -> String {
    if regions == 1 {
        "image 1 is the whole frame at reduced size; image 2 is the full-resolution crop of region r1".into()
    } else {
        format!(
            "image 1 is the whole frame at reduced size; images 2..{} are the full-resolution crops of regions r1..r{regions} in that order",
            regions + 1
        )
    }
}

fn label(position: usize) -> String {
    format!("r{}", position + 1)
}

fn rounded(value: f64, scale: f64) -> f64 {
    (value * scale).round() / scale
}

fn translation(english: Option<&str>) -> Option<String> {
    english
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// Four finite normalised coordinates with a positive width and height.
fn valid_box(bbox: [f64; 4]) -> bool {
    bbox.iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        && bbox[2] > bbox[0]
        && bbox[3] > bbox[1]
}

fn intersection_over_union(a: [f64; 4], b: [f64; 4]) -> f64 {
    let width = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let height = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    let intersection = width * height;
    let union = (a[2] - a[0]) * (a[3] - a[1]) + (b[2] - b[0]) * (b[3] - b[1]) - intersection;
    if union > 0.0 {
        intersection / union
    } else {
        0.0
    }
}

#[cfg(test)]
#[path = "tests/keyframe_requests.rs"]
mod tests;
