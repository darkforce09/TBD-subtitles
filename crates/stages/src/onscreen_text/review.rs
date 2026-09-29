//! Apply owner edits to visible text without changing dialogue.
//!
//! **Role:** validate times and presentation, keep originals available for undo.
//! **Position:** fifth visual stage, before typesetting.
//! **Signals and state:** immutable corrections and a mutable text document.
//! **Invariants:** invalid corrections fail explicitly; edits never alter spoken words.

use std::collections::HashSet;

use super::TextResult;
use job_model::onscreen::{TextCorrections, TextDocument, TextFrame, TextTreatment};

const WARNING_PREFIX: &str = "Visual correction not applied:";

pub fn apply(
    document: &mut TextDocument,
    corrections: &TextCorrections,
    duration_s: f64,
) -> TextResult<()> {
    document
        .review_warnings
        .retain(|warning| !warning.starts_with(WARNING_PREFIX));
    let ids: HashSet<_> = document
        .occurrences
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    for id in corrections
        .edits
        .keys()
        .filter(|id| !ids.contains(id.as_str()))
    {
        document.review_warnings.push(format!(
            "{WARNING_PREFIX} {id} has no matching occurrence after reprocessing. Reopen Check Text and remove or recreate this correction."
        ));
    }
    for item in &mut document.occurrences {
        item.warnings
            .retain(|warning| !warning.starts_with(WARNING_PREFIX));
        if !item.reviewed {
            item.source_fingerprint = Some(item.observation_fingerprint());
        }
        if let Some(edit) = corrections.edits.get(&item.id) {
            let identity_matches = edit
                .source_fingerprint
                .as_ref()
                .zip(item.source_fingerprint.as_ref())
                .is_some_and(|(expected, observed)| expected == observed);
            if !identity_matches {
                let reason =
                    if edit.source_fingerprint.is_none() || item.source_fingerprint.is_none() {
                        "the original source identity is missing"
                    } else {
                        "the observed Japanese, timing, geometry or crop identity changed"
                    };
                // Older edited artifacts no longer contain their original English.
                if item.reviewed {
                    item.english = None;
                }
                item.reviewed = false;
                item.rendered = None;
                item.warnings.push(format!(
                    "{WARNING_PREFIX} {}: {reason}. Regenerate visual processing and save the correction against the current occurrence.", item.id
                ));
                continue;
            }
            edit.validate(duration_s)?;
            let first = item.frames.first().cloned();
            let last = item.frames.last().cloned();
            item.start_s = edit.start_s;
            item.end_s = edit.end_s;
            item.english = edit.english.clone().filter(|text| !text.trim().is_empty());
            item.presentation = edit.presentation.clone();
            item.reviewed = true;
            item.frames
                .retain(|frame| frame.end_s > edit.start_s && frame.time_s < edit.end_s);
            for frame in &mut item.frames {
                frame.time_s = frame.time_s.max(edit.start_s);
                frame.end_s = frame.end_s.min(edit.end_s);
            }
            // Extending a tracked span has no verified geometry: keep it as a nearby annotation.
            if let Some(first) = first
                && edit.start_s < first.time_s
            {
                item.frames.insert(
                    0,
                    TextFrame {
                        time_s: edit.start_s,
                        end_s: first.time_s.min(edit.end_s),
                        ..first
                    },
                );
                item.presentation.treatment = TextTreatment::Nearby;
            }
            if let Some(last) = last
                && edit.end_s > last.end_s
            {
                item.frames.push(TextFrame {
                    time_s: last.end_s.max(edit.start_s),
                    end_s: edit.end_s,
                    ..last
                });
                item.presentation.treatment = TextTreatment::Nearby;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/review.rs"]
mod tests;
