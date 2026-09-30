//! What Check Text knows about a job that replaces writing in a localized video.
//!
//! **Role:** hold the localized video and its subtitle file as they stand, each occurrence's
//! replacement (drawn in, left out with the reason, or not decided yet), which picture the right
//! preview shows, whether the erase mask shows, and the selected occurrence's pictures.
//! **Position:** part of the text review session; filled by `services::localized`, drawn by the
//! preview.
//! **Signals and state:** plain data; paths are absolute.
//! **Invariants:** a job that writes no localized video has none of this; the pictures belong to
//! the occurrence they name.

use std::collections::BTreeMap;
use std::path::PathBuf;

use job_model::onscreen::{PixelRect, ReplaceStatus};

use super::Picture;

/// The picture on the right of the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreviewMode {
    /// The source video with the exported subtitle file burned in.
    Subtitles,
    /// The localized video with its own subtitle file burned in.
    Localized,
}

/// A job's localized video as Check Text shows it.
#[derive(Debug, Clone)]
pub(crate) struct LocalizedReview {
    /// The localized video, once written.
    pub(crate) video: Option<PathBuf>,
    /// The localized video's subtitle file, once written.
    pub(crate) subtitles: Option<PathBuf>,
    /// Each occurrence's replacement, by occurrence id.
    pub(crate) replacements: BTreeMap<String, Replacement>,
    pub(crate) mode: PreviewMode,
    /// The erase mask shows over the original picture.
    pub(crate) show_mask: bool,
    /// The selected occurrence's replaced plate and erase mask, once loaded.
    pub(crate) pictures: Option<ReplacementPictures>,
}

/// One occurrence's replacement.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Replacement {
    pub(crate) status: ReplaceStatus,
    /// The keyframe plate after replacement (RGB PNG).
    pub(crate) preview: Option<PathBuf>,
    /// The keyframe plate's erase mask and where it sits.
    pub(crate) mask: Option<MaskPlate>,
}

/// An erase mask (8-bit PNG, 255 erases) and the plate rectangle it covers, in source pixels.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MaskPlate {
    pub(crate) rect: PixelRect,
    pub(crate) path: PathBuf,
}

/// The selected occurrence's pictures, decoded off the window thread.
#[derive(Debug, Clone)]
pub(crate) struct ReplacementPictures {
    /// The occurrence they belong to.
    pub(crate) id: String,
    pub(crate) preview: Option<Picture>,
    pub(crate) mask: Option<Mask>,
}

/// An erase mask ready to overlay: its coverage (0 keeps, 255 erases) and where it sits.
#[derive(Debug, Clone)]
pub(crate) struct Mask {
    pub(crate) serial: u64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) coverage: Vec<u8>,
    pub(crate) rect: PixelRect,
}
