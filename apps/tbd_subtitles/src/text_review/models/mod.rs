//! Plain state and events for reviewing on-screen text.
//!
//! **Role:** hold a loaded visual document, draft edit, preview position and, for a job that
//! writes a localized video, its replacements.
//! **Position:** shared by the text review services and rendering.
//! **Signals and state:** no rendering or filesystem operations.
//! **Invariants:** source observations remain immutable while the draft is edited.

use job_model::onscreen::{TextCorrections, TextDocument, TextEdit};
use std::path::PathBuf;

mod localized;

pub(crate) use localized::{
    LocalizedReview, Mask, MaskPlate, PreviewMode, Replacement, ReplacementPictures,
};

#[derive(Debug, Clone)]
pub(crate) struct Session {
    pub(crate) work: PathBuf,
    pub(crate) video: PathBuf,
    pub(crate) ass: PathBuf,
    pub(crate) document: TextDocument,
    pub(crate) corrections: TextCorrections,
    pub(crate) duration_s: f64,
    pub(crate) fps: f64,
    pub(crate) audio_position: u32,
    pub(crate) selected: usize,
    pub(crate) draft: Option<TextEdit>,
    pub(crate) position_s: f64,
    pub(crate) flagged_only: bool,
    pub(crate) error: Option<String>,
    pub(crate) thumbnails: Vec<Option<Picture>>,
    /// The localized video, when the job writes one.
    pub(crate) localized: Option<LocalizedReview>,
}

#[derive(Debug, Clone)]
pub(crate) struct Picture {
    pub(crate) serial: u64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgb: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct Comparison {
    pub(crate) original: Picture,
    /// The right picture; none while it shows the localized video before it is written.
    pub(crate) rendered: Option<Picture>,
    pub(crate) time_s: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event {
    Select(usize),
    Edit(TextEdit),
    Save,
    Undo,
    Retry,
    DiscardOrphans,
    Seek(f64),
    Play,
    Stop,
    FlaggedOnly(bool),
    /// Show the source with its subtitles, or the localized video, on the right.
    PreviewMode(PreviewMode),
    /// Show the erase mask over the original picture.
    ShowMask(bool),
}
