//! The corrections, `review.json`: written by the window's line review and by Fix It, read by the
//! review step and the quality check.
//!
//! **Role:** hold each corrected utterance's text and flags and where its text came from: an
//! engine, the owner's typing, or Fix It, checked by the owner or not.
//!
//! **Position:** written through `pipeline::work_dir::update_corrections`; read by the review
//! step, the quality check, Fix It and the window.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** at most one correction per utterance; a correction is the owner's unless it is
//! a Fix It change the owner has not kept; a file written before Fix It reads unchanged.

use serde::{Deserialize, Serialize};

/// Where a corrected text came from.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Chosen {
    /// One engine's hypothesis, by its tag (`P`, `W`, `ALT p`, `ALT w`) or `adjudicated`.
    Engine(String),
    /// Text the owner typed.
    Typed,
    /// Fix It's change, not checked by the owner yet: the `claude` model that made it and why.
    FixIt { model: String, why: String },
    /// A Fix It change the owner kept; it is the owner's from then on.
    KeptFixIt { model: String, why: String },
}

impl Chosen {
    /// Whether Fix It wrote it and the owner has not kept it yet.
    pub fn is_unchecked_fix(&self) -> bool {
        matches!(self, Chosen::FixIt { .. })
    }
}

/// One corrected utterance.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
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

    /// Whether the line is the owner's: chosen, typed, kept as it was, or a kept Fix It change.
    pub fn by_owner(&self) -> bool {
        !self.chosen.is_unchecked_fix()
    }
}

/// Every correction of one job, at most one per utterance.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
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

    /// Whether the owner settled line `id`; a Fix It change the owner has not kept does not count.
    pub fn by_owner(&self, id: &str) -> bool {
        self.get(id).is_some_and(Correction::by_owner)
    }

    /// How many lines the owner settled.
    pub fn owner_count(&self) -> usize {
        self.lines.iter().filter(|c| c.by_owner()).count()
    }

    /// How many Fix It changes wait for the owner.
    pub fn unchecked_fix_count(&self) -> usize {
        self.lines.len() - self.owner_count()
    }
}

#[cfg(test)]
#[path = "tests/review.rs"]
mod tests;
