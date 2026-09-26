//! The owner's corrections, `review.json`: written by the window's line review, read by the
//! review step and the quality check.

use serde::{Deserialize, Serialize};

/// Where a corrected text came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Chosen {
    /// One engine's hypothesis, by its tag (`P`, `W`, `ALT p`, `ALT w`) or `adjudicated`.
    Engine(String),
    /// Text the owner typed.
    Typed,
}

/// One corrected utterance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Correction {
    /// The utterance id, such as `U0412`.
    pub id: String,
    /// The final text; `||` starts another speaker.
    pub text: String,
    /// The line's flags after the correction (`NARR`, `SPK`, `LYRIC`, `DROP`); `UNSURE` is gone.
    #[serde(default)]
    pub flags: Vec<String>,
    pub chosen: Chosen,
}

impl Correction {
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }
}

/// Every correction of one job, at most one per utterance.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Corrections {
    pub lines: Vec<Correction>,
}

impl Corrections {
    pub fn get(&self, id: &str) -> Option<&Correction> {
        self.lines.iter().find(|c| c.id == id)
    }

    /// Put `correction` in, replacing any earlier one of the same utterance.
    pub fn set(&mut self, correction: Correction) {
        match self.lines.iter_mut().find(|c| c.id == correction.id) {
            Some(slot) => *slot = correction,
            None => self.lines.push(correction),
        }
    }

    /// Take the correction of `id` out; `false` when there was none.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.lines.len();
        self.lines.retain(|c| c.id != id);
        self.lines.len() != before
    }
}
