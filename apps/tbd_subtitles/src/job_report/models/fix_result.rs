//! What Fix It did to a finished job, as the Overview's result card says it.
//!
//! **Role:** hold the counts, the problems cleared and left, and a few changed words of the
//! latest Fix It run of a job, read from its stored record and its corrections.
//!
//! **Position:** built by `services::fix_result`, kept in `JobReport::fix_result`, drawn by the
//! file card's result card.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** `examples` holds at most two changes and `more` the changed lines past them,
//! so `examples.len() + more == changed` whenever every changed line changed its words; a change
//! reads as words replaced, put in or taken out (`change_words`), never as a diff.

use crate::job_report::models::problem::Problem;

/// One job's Fix It outcome.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct FixResult {
    /// The model as the window names it, such as "Claude Opus".
    pub(crate) model: String,
    /// The lines whose change went into the subtitles.
    pub(crate) changed: usize,
    /// The lines Claude answered and left as they were.
    pub(crate) already_right: usize,
    /// The lines whose change the judge turned down.
    pub(crate) turned_down: usize,
    /// The lines Claude would have changed that keep the owner's own correction.
    pub(crate) kept_yours: usize,
    /// Claude's changes the owner kept.
    pub(crate) owner_kept: usize,
    /// Claude's changes the owner undid.
    pub(crate) owner_undid: usize,
    /// The problems before the first Fix It run that the job no longer has.
    pub(crate) cleared: Vec<Problem>,
    /// The problems the job still has.
    pub(crate) left: Vec<Problem>,
    /// The words of the first changes, before then after as written: `""` before for words put
    /// in, `""` after for words taken out.
    pub(crate) examples: Vec<(String, String)>,
    /// The changed lines not among the examples.
    pub(crate) more: usize,
}

/// A change's words as the owner reads them at a glance: `“Heaven dish” → “Cavendish”` for words
/// replaced, `Added “Uh,”` for words put in, `Removed “So”` for words taken out.
pub(crate) fn change_words(before: &str, after: &str) -> String {
    match (before.is_empty(), after.is_empty()) {
        (true, _) => format!("Added “{after}”"),
        (_, true) => format!("Removed “{before}”"),
        _ => format!("“{before}” → “{after}”"),
    }
}

#[cfg(test)]
#[path = "tests/fix_result.rs"]
mod tests;
