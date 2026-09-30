//! Contracts for the read-back check of lettered replacements.
//!
//! **Role:** carry the replacements with their final statuses, and what a local OCR read back
//! from the finished picture of each checked occurrence, to the localized video, the subtitle
//! layout, the report and the desktop window.
//! **Position:** bottom-layer data written by the `text_verify` step; `visual/text_verify.json`
//! holds one `VerifiedReplacements`, whose replacement fields sit at the top level so the file
//! also reads as a plain `ReplacementDocument`.
//! **Signals and state:** serializable values; no I/O.
//! **Invariants:** every check names an occurrence of the document; a checked occurrence stays
//! baked only when every one of its readings passed.

use serde::{Deserialize, Serialize};

use super::ReplacementDocument;

/// What the OCR found in one finished frame of an occurrence.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VerifyReading {
    /// The frame index read, on the decoded timeline.
    pub frame: u64,
    /// Readings that still hold Japanese, joined by spaces; empty when none do.
    pub japanese_found: String,
    /// Everything read over the lettering, top to bottom and left to right, joined by spaces.
    pub english_read: String,
    /// How closely the letters and digits read match the English, from 0 to 1.
    pub similarity: f64,
    /// Whether this frame passed both checks.
    pub passed: bool,
}

/// The readings of one checked occurrence, in frame order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TextCheck {
    /// The `TextOccurrence` id.
    pub id: String,
    pub readings: Vec<VerifyReading>,
}

impl TextCheck {
    /// The reading that says most about the occurrence: the first that failed, else the one that
    /// matched the English least.
    pub fn telling(&self) -> Option<&VerifyReading> {
        self.readings
            .iter()
            .find(|reading| !reading.passed)
            .or_else(|| {
                self.readings
                    .iter()
                    .min_by(|a, b| a.similarity.total_cmp(&b.similarity))
            })
    }
}

/// The replacements after the read-back check, with what it read.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VerifiedReplacements {
    #[serde(flatten)]
    pub document: ReplacementDocument,
    #[serde(default)]
    pub checks: Vec<TextCheck>,
}

impl VerifiedReplacements {
    /// The check of occurrence `id`, when it was checked.
    pub fn check(&self, id: &str) -> Option<&TextCheck> {
        self.checks.iter().find(|check| check.id == id)
    }
}

#[cfg(test)]
#[path = "tests/verify.rs"]
mod tests;
