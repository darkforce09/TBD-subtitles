//! Which tracked files the link check judges, and the area each belongs to.
//!
//! **Role:** selects the judged documents — every Markdown file under the documentation root,
//! every README.md anywhere, and the project instructions — and files each in one area for the
//! totals, telling frozen records from live documents.
//!
//! **Position:** called by the gate in [`super::super::link_check`] for every tracked file; the
//! locations come from [`crate::layout`].
//!
//! **Signals and state:** none; pure functions over repository-relative paths.
//!
//! **Invariants:** every judged file lands in exactly one area, the first that holds it in the
//! order of [`DocumentArea::ALL`]; a document other than a README.md inside a frozen folder is a
//! frozen record.

use crate::gate_run::path_regions::{README, file_name, is_frozen_record, is_markdown, is_within};
use crate::layout::{DOCUMENTATION_ROOT, PROJECT_INSTRUCTIONS};

/// Where a judged document sits, for the totals and for the rules that skip frozen records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DocumentArea {
    /// A live document under the documentation root.
    LiveDocumentation,
    /// A research snapshot or an archived document: frozen, only its links change.
    FrozenDocumentation,
    /// The project instructions at the repository root.
    ProjectInstructions,
    /// A README.md outside the documentation root, the repository root's included.
    Readmes,
}

impl DocumentArea {
    /// Every area, in the order a path is matched against them and the totals list them.
    pub(crate) const ALL: [DocumentArea; 4] = [
        DocumentArea::LiveDocumentation,
        DocumentArea::FrozenDocumentation,
        DocumentArea::ProjectInstructions,
        DocumentArea::Readmes,
    ];

    /// The area's name in the totals.
    pub(crate) fn label(self) -> String {
        match self {
            DocumentArea::LiveDocumentation => format!("{DOCUMENTATION_ROOT} live documents"),
            DocumentArea::FrozenDocumentation => format!("{DOCUMENTATION_ROOT} frozen records"),
            DocumentArea::ProjectInstructions => PROJECT_INSTRUCTIONS.to_string(),
            DocumentArea::Readmes => format!("{README} files elsewhere"),
        }
    }

    /// Whether the area holds frozen records, which only the link rules judge.
    pub(crate) fn is_frozen(self) -> bool {
        self == DocumentArea::FrozenDocumentation
    }
}

/// The area of a tracked file the link check judges, or `None` when it judges no such file.
pub(crate) fn judged_area(path: &str) -> Option<DocumentArea> {
    if is_within(path, DOCUMENTATION_ROOT) {
        if !is_markdown(path) {
            return None;
        }
        return Some(if is_frozen_record(path) {
            DocumentArea::FrozenDocumentation
        } else {
            DocumentArea::LiveDocumentation
        });
    }
    if path == PROJECT_INSTRUCTIONS {
        return Some(DocumentArea::ProjectInstructions);
    }
    (file_name(path) == README).then_some(DocumentArea::Readmes)
}

#[cfg(test)]
#[path = "tests/judged_documents.rs"]
mod tests;
