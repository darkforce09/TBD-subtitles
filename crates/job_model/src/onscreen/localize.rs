//! Contracts for in-place replacement: stroke masks, inpainted plates, composed patches and the
//! localized video they are blended into.
//!
//! **Role:** carry each replaceable occurrence from mask extraction through inpainting and
//! typography to the localized video encoder.
//! **Position:** bottom-layer data shared by stages, pipeline and the desktop window.
//! **Signals and state:** serializable values; no I/O.
//! **Invariants:** frame numbers index the video's decoded frame timeline from zero; rectangles
//! use source pixels and stay inside the frame; paths are relative to the job directory; a
//! plate's frame span lies inside its occurrence's span and plates of one occurrence never
//! overlap.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::Quad;

/// An axis-aligned rectangle of source pixels.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub fn right(self) -> u32 {
        self.x + self.width
    }

    pub fn bottom(self) -> u32 {
        self.y + self.height
    }

    pub fn area(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// Whether the rectangle is non-empty and inside a `width` × `height` frame.
    pub fn inside(self, width: u32, height: u32) -> bool {
        self.width > 0 && self.height > 0 && self.right() <= width && self.bottom() <= height
    }

    /// Whether the two rectangles share at least one pixel.
    pub fn overlaps(self, other: PixelRect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

/// Whether an occurrence is drawn into the localized video or left in Japanese.
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
#[serde(rename_all = "snake_case", tag = "kind", content = "reason")]
pub enum ReplaceStatus {
    /// Not yet decided by a later step.
    #[default]
    Pending,
    /// Erased, inpainted and redrawn in English in the localized video.
    Baked,
    /// Left in the picture as it is, with the reason.
    Fallback(String),
}

/// The lettering style measured from the original strokes before they are erased.
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
pub struct LetteringStyle {
    /// Dominant colour of the glyph interiors.
    pub fill_rgb: [u8; 3],
    /// Colour of an outline or halo around the glyphs, when one is present.
    pub outline_rgb: Option<[u8; 3]>,
    /// Outline thickness in source pixels; zero without an outline.
    pub outline_px: f64,
    /// The outline fades out rather than ending in a hard edge.
    pub soft_outline: bool,
    /// Typical stroke thickness in source pixels.
    pub stroke_px: f64,
    /// Height of one line of the original writing in source pixels.
    pub line_height_px: f64,
}

/// A run of consecutive frames that share one background plate.
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
pub struct Plate {
    /// First frame of the run, inclusive.
    pub first_frame: u64,
    /// Last frame of the run, inclusive.
    pub last_frame: u64,
    /// Where the plate sits in the frame.
    pub rect: PixelRect,
    /// Movement of the writing relative to the keyframe quad at this run: pixels.
    pub shift: [f64; 2],
    /// Scale of the writing relative to the keyframe quad at this run.
    pub scale: f64,
    /// The original pixels of `rect` at `first_frame` (RGB PNG).
    #[rkyv(with = rkyv::with::AsString)]
    pub source: PathBuf,
    /// The erase mask for `rect` (8-bit PNG: 255 erases, 0 keeps).
    #[rkyv(with = rkyv::with::AsString)]
    pub mask: PathBuf,
    /// The inpainted pixels of `rect` (RGB PNG), once inpainting has run.
    #[serde(default)]
    #[rkyv(with = rkyv::with::Map<rkyv::with::AsString>)]
    pub plate: Option<PathBuf>,
    /// The English lettering composed onto the plate (RGBA PNG), once composition has run; alpha
    /// covers the erased strokes and the new lettering.
    #[serde(default)]
    #[rkyv(with = rkyv::with::Map<rkyv::with::AsString>)]
    pub patch: Option<PathBuf>,
}

/// One occurrence's replacement.
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
pub struct ReplacedText {
    /// The `TextOccurrence` id.
    pub id: String,
    /// First frame of the occurrence, inclusive.
    pub first_frame: u64,
    /// Last frame of the occurrence, inclusive.
    pub last_frame: u64,
    pub status: ReplaceStatus,
    #[serde(default)]
    pub style: Option<LetteringStyle>,
    /// Occurrences drawn together as one card share a container id.
    #[serde(default)]
    pub container: Option<String>,
    #[serde(default)]
    pub plates: Vec<Plate>,
    /// The keyframe with the composed replacement (RGB PNG) for review.
    #[serde(default)]
    #[rkyv(with = rkyv::with::Map<rkyv::with::AsString>)]
    pub preview: Option<PathBuf>,
    /// Where the English is lettered at the keyframe when the occurrence's own quad is only a
    /// loose box around the writing: the bounds of the ink the mask erases, in source pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lettering_quad: Option<Quad>,
}

/// The replacements of a job, written by the mask, inpaint and compose steps in turn.
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
pub struct ReplacementDocument {
    pub width: u32,
    pub height: u32,
    /// Frames in the decoded timeline.
    pub frame_count: u64,
    pub texts: Vec<ReplacedText>,
}

impl ReplacementDocument {
    /// Occurrences drawn into the localized video.
    pub fn baked(&self) -> impl Iterator<Item = &ReplacedText> {
        self.texts
            .iter()
            .filter(|text| text.status == ReplaceStatus::Baked)
    }

    /// Check frame spans, plate spans and rectangles against the invariants.
    pub fn validate(&self) -> Result<(), String> {
        for text in &self.texts {
            if text.first_frame > text.last_frame || text.last_frame >= self.frame_count {
                return Err(format!("{}: frame span outside the video", text.id));
            }
            let mut previous: Option<u64> = None;
            for plate in &text.plates {
                if plate.first_frame > plate.last_frame
                    || plate.first_frame < text.first_frame
                    || plate.last_frame > text.last_frame
                    || previous.is_some_and(|last| plate.first_frame <= last)
                {
                    return Err(format!("{}: plate frames out of order", text.id));
                }
                if !plate.rect.inside(self.width, self.height) {
                    return Err(format!("{}: plate outside the frame", text.id));
                }
                if !(plate.scale.is_finite()
                    && plate.scale > 0.0
                    && plate.shift.iter().all(|v| v.is_finite()))
                {
                    return Err(format!("{}: invalid plate placement", text.id));
                }
                previous = Some(plate.last_frame);
            }
        }
        Ok(())
    }
}

/// What the localized-video step produced.
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
pub struct LocalizedVideoRecord {
    /// The localized video beside the source, when one was written.
    pub path: Option<String>,
    /// The encoder FFmpeg used, e.g. `hevc_nvenc` or `libx264`.
    pub encoder: String,
    pub frames: u64,
    /// Occurrences drawn into the video.
    pub replaced: usize,
    /// A localized video this job wrote on an earlier run and left beside the source, kept so a
    /// later run may replace it.
    #[serde(default)]
    pub earlier: Option<String>,
}

#[cfg(test)]
#[path = "tests/localize.rs"]
mod tests;
