//! Settings for the visible-text branch.
//!
//! **Role:** separate new-job defaults from compatibility defaults for saved jobs.
//! **Position:** consumed by job settings, GUI settings and step fingerprints.
//! **Signals and state:** plain serializable data.
//! **Invariants:** absent settings in an old job disable the branch and the localized video; new
//! jobs enable both.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextSettings {
    pub enabled: bool,
    pub claude_fallback: bool,
    pub reference_folder: Option<PathBuf>,
    /// Erase replaceable writing and draw its English into `<video>.localized.mkv`.
    pub localized_video: bool,
}

impl Default for TextSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            claude_fallback: true,
            reference_folder: None,
            localized_video: false,
        }
    }
}

impl TextSettings {
    pub fn new_job() -> Self {
        Self {
            enabled: true,
            localized_video: true,
            ..Self::default()
        }
    }
}
