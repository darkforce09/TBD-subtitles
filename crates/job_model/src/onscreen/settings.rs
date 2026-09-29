//! Settings for the visible-text branch.
//!
//! **Role:** separate new-job defaults from compatibility defaults for saved jobs.
//! **Position:** consumed by job settings, GUI settings and step fingerprints.
//! **Signals and state:** plain serializable data.
//! **Invariants:** absent settings in an old job disable the branch; new jobs enable it.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextSettings {
    pub enabled: bool,
    pub claude_fallback: bool,
    pub reference_folder: Option<PathBuf>,
}

impl Default for TextSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            claude_fallback: true,
            reference_folder: None,
        }
    }
}

impl TextSettings {
    pub fn new_job() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}
