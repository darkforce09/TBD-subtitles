//! Plain state and events for reviewing on-screen text.
//!
//! **Role:** hold a loaded visual document, draft edit and preview position.
//! **Position:** shared by the text review services and rendering.
//! **Signals and state:** no rendering or filesystem operations.
//! **Invariants:** source observations remain immutable while the draft is edited.

use job_model::onscreen::{TextCorrections, TextDocument, TextEdit};
use std::path::PathBuf;

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
    pub(crate) rendered: Picture,
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
}
