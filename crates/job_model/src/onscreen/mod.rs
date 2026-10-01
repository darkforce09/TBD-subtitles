//! Contracts for visible writing and its English presentation.
//!
//! **Role:** carry observations, tracks, translations and owner corrections between steps.
//! **Position:** bottom-layer data shared by media, inference, stages, pipeline and GUI.
//! **Signals and state:** serializable values; no I/O.
//! **Invariants:** times use the video's presentation timeline; coordinates use source pixels;
//! unreadable writing has no invented English translation.

mod frames;
mod localize;
mod settings;
mod verify;

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use frames::{FrameRecord, RleRun, decode_mask, encode_mask, mask_area};
pub use localize::{
    LetteringStyle, LocalizedVideoRecord, PixelRect, Plate, ReplaceStatus, ReplacedText,
    ReplacementDocument, ShiftedPatch,
};
pub use settings::TextSettings;
pub use verify::{TextCheck, VerifiedReplacements, VerifyReading, telling};

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Clockwise corners: top left, top right, bottom right, bottom left.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct Quad(pub [Point; 4]);

impl Quad {
    pub fn bounds(self) -> (f64, f64, f64, f64) {
        self.0.iter().fold(
            (
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ),
            |(l, t, r, b), p| (l.min(p.x), t.min(p.y), r.max(p.x), b.max(p.y)),
        )
    }

    pub fn center(self) -> Point {
        let (l, t, r, b) = self.bounds();
        Point {
            x: (l + r) / 2.0,
            y: (t + b) / 2.0,
        }
    }

    pub fn valid(self) -> bool {
        let (l, t, r, b) = self.bounds();
        self.0.iter().all(|p| p.x.is_finite() && p.y.is_finite()) && r > l && b > t
    }
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct TextFrame {
    pub time_s: f64,
    pub end_s: f64,
    pub quad: Quad,
    pub confidence: f64,
    /// A conservative estimate, never permission to cover uncertain foreground objects.
    pub surface_rgb: Option<[u8; 3]>,
}

#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[rkyv(compare(PartialEq), derive(Debug, PartialEq, Eq))]
pub enum TextTreatment {
    #[default]
    Auto,
    Replace,
    Nearby,
}

#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct TextPresentation {
    pub treatment: TextTreatment,
    pub anchor: Option<Point>,
    pub font_size: Option<f64>,
}

#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct TextProvenance {
    pub backend: String,
    #[rkyv(with = rkyv::with::Map<rkyv::with::AsString>)]
    pub reference: Option<PathBuf>,
    pub reason: String,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct TextOccurrence {
    pub id: String,
    pub start_s: f64,
    pub end_s: f64,
    pub japanese: String,
    pub english: Option<String>,
    pub confidence: f64,
    /// Representative crops relative to the job directory.
    #[rkyv(with = rkyv::with::Map<rkyv::with::AsString>)]
    pub crops: Vec<PathBuf>,
    pub frames: Vec<TextFrame>,
    pub provenance: TextProvenance,
    pub presentation: TextPresentation,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub reviewed: bool,
    /// None before typesetting or in older records; false means English could not be displayed.
    #[serde(default)]
    pub rendered: Option<bool>,
    /// Identity of the unedited observation, preserved after review trims or extends its frames.
    #[serde(default)]
    pub source_fingerprint: Option<String>,
    /// The observed frame that represents the occurrence and the whole-frame still taken there.
    #[serde(default)]
    pub keyframe: Option<TextKeyframe>,
    /// Furigana lines folded into this line: erased with it and never lettered on their own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ruby: Vec<Quad>,
}

/// One observed frame of an occurrence with its downscaled whole-frame still, relative to the
/// job directory, for image requests that need the surrounding picture.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct TextKeyframe {
    pub time_s: f64,
    #[rkyv(with = rkyv::with::AsString)]
    pub image: PathBuf,
}

impl TextOccurrence {
    /// Hash only original observed content, crop identity and per-frame geometry/timing.
    pub fn observation_fingerprint(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(b"tbd-visible-text-source-v1\0");
        hash.update((self.japanese.len() as u64).to_le_bytes());
        hash.update(self.japanese.as_bytes());
        hash.update((self.crops.len() as u64).to_le_bytes());
        for crop in &self.crops {
            let bytes = crop.as_os_str().as_encoded_bytes();
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        hash.update((self.frames.len() as u64).to_le_bytes());
        if self.frames.is_empty() {
            hash.update(self.start_s.to_bits().to_le_bytes());
            hash.update(self.end_s.to_bits().to_le_bytes());
        }
        for frame in &self.frames {
            for value in [frame.time_s, frame.end_s]
                .into_iter()
                .chain(frame.quad.0.iter().flat_map(|point| [point.x, point.y]))
            {
                hash.update(value.to_bits().to_le_bytes());
            }
        }
        format!("v1-{:x}", hash.finalize())
    }
}

#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct TextDocument {
    pub width: u32,
    pub height: u32,
    pub decoded_frames: u64,
    pub occurrences: Vec<TextOccurrence>,
    /// Review diagnostics without a current occurrence, including orphaned corrections.
    #[serde(default)]
    pub review_warnings: Vec<String>,
    /// Width of the screening copy the scan ran on; zero in documents from other sources.
    #[serde(default)]
    pub proxy_width: u32,
    /// Frames between coarse samples; zero in documents from other sources.
    #[serde(default)]
    pub sample_step: u32,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct TextEdit {
    pub english: Option<String>,
    pub start_s: f64,
    pub end_s: f64,
    pub presentation: TextPresentation,
    /// The original observation this edit belongs to; older unbound edits need fresh review.
    #[serde(default)]
    pub source_fingerprint: Option<String>,
}

impl TextEdit {
    pub fn matches_source(&self, item: &TextOccurrence) -> bool {
        self.source_fingerprint.as_ref().is_some_and(|expected| {
            expected
                == &item
                    .source_fingerprint
                    .clone()
                    .unwrap_or_else(|| item.observation_fingerprint())
        })
    }
    pub fn from_occurrence(text: &TextOccurrence) -> Self {
        Self {
            english: text.english.clone(),
            start_s: text.start_s,
            end_s: text.end_s,
            presentation: text.presentation.clone(),
            source_fingerprint: text
                .source_fingerprint
                .clone()
                .or_else(|| (!text.reviewed).then(|| text.observation_fingerprint())),
        }
    }

    pub fn validate(&self, duration_s: f64) -> Result<(), String> {
        if !self.start_s.is_finite()
            || !self.end_s.is_finite()
            || self.start_s < 0.0
            || self.end_s <= self.start_s
            || self.end_s > duration_s
        {
            return Err(
                "Text timing must be inside the video, with its end after its start.".into(),
            );
        }
        if self
            .presentation
            .font_size
            .is_some_and(|v| !v.is_finite() || !(8.0..=400.0).contains(&v))
        {
            return Err("Text size must be between 8 and 400 pixels.".into());
        }
        if self
            .presentation
            .anchor
            .is_some_and(|p| !p.x.is_finite() || !p.y.is_finite())
        {
            return Err("Text position must contain finite coordinates.".into());
        }
        Ok(())
    }
}

#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct TextCorrections {
    pub edits: BTreeMap<String, TextEdit>,
    /// Occurrences whose OCR and translation must be retried on the next run.
    #[serde(default)]
    pub retry: Vec<String>,
}

#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct TextSummary {
    pub detected: usize,
    pub translated: usize,
    pub fallback: usize,
    pub unresolved: usize,
    pub flagged: usize,
}

impl TextDocument {
    pub fn summary(&self) -> TextSummary {
        TextSummary {
            detected: self.occurrences.len(),
            translated: self
                .occurrences
                .iter()
                .filter(|t| t.english.is_some())
                .count(),
            unresolved: self
                .occurrences
                .iter()
                .filter(|t| t.english.is_none() || t.rendered == Some(false))
                .count(),
            fallback: self
                .occurrences
                .iter()
                .filter(|t| t.presentation.treatment == TextTreatment::Nearby)
                .count(),
            flagged: self
                .occurrences
                .iter()
                .filter(|t| {
                    t.rendered == Some(false)
                        || (!t.reviewed && (!t.warnings.is_empty() || t.english.is_none()))
                })
                .count()
                + self.review_warnings.len(),
        }
    }
}

#[cfg(test)]
#[path = "tests/contracts.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/archive.rs"]
mod archive_tests;
