//! Occurrences the sign library already holds: their translation taken from the approved sign.
//!
//! **Role:** keep a keyframe whose every occurrence is a known sign out of the Claude requests,
//! and give each known occurrence the approved sign's reading, English and confidence in place of
//! whatever a request answered for it.
//! **Position:** private helper of the visual translation stage; the pipeline finds the signs.
//! **Signals and state:** the signs by occurrence id; no I/O.
//! **Invariants:** a known occurrence ends exactly as it entered the stage but for the sign's
//! reading, English, confidence and a `library` provenance; with no known sign nothing changes.

use super::keyframe_requests::Request;
use job_model::onscreen::{LibrarySign, TextDocument, TextOccurrence, TextProvenance};
use std::collections::BTreeMap;

/// The provenance backend of a translation taken from the sign library.
pub const LIBRARY_BACKEND: &str = "library";

/// Drop every request whose occurrences are all known signs.
pub(super) fn skip_known(requests: &mut Vec<Request>, known: &BTreeMap<String, LibrarySign>) {
    if known.is_empty() {
        return;
    }
    requests.retain(|request| {
        !request
            .regions
            .iter()
            .all(|region| known.contains_key(&region.id))
    });
}

/// The known occurrences of `document` by index, as they entered the stage.
pub(super) fn snapshot(
    document: &TextDocument,
    known: &BTreeMap<String, LibrarySign>,
) -> Vec<(usize, TextOccurrence)> {
    document
        .occurrences
        .iter()
        .enumerate()
        .filter(|(_, item)| known.contains_key(&item.id))
        .map(|(index, item)| (index, item.clone()))
        .collect()
}

/// Put each snapshot occurrence back with its sign's translation; the reason its review warning
/// would quote goes into `reasons`.
pub(super) fn apply(
    document: &mut TextDocument,
    entered: Vec<(usize, TextOccurrence)>,
    known: &BTreeMap<String, LibrarySign>,
    reasons: &mut [Option<String>],
) {
    for (index, mut item) in entered {
        let Some(sign) = known.get(&item.id) else {
            continue;
        };
        let first = sign.origin().unwrap_or("an earlier job");
        let reason = format!("Translation: the approved sign from {first}.");
        item.japanese.clone_from(&sign.japanese);
        item.english = Some(sign.english.clone());
        item.confidence = sign.confidence;
        item.provenance = TextProvenance {
            backend: LIBRARY_BACKEND.into(),
            reference: None,
            reason: if item.provenance.reason.is_empty() {
                reason.clone()
            } else {
                format!("{} {reason}", item.provenance.reason)
            },
        };
        document.occurrences[index] = item;
        reasons[index] = Some(reason);
    }
}

#[cfg(test)]
#[path = "tests/known_signs.rs"]
mod tests;
