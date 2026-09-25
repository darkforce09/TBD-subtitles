//! The anchors a rendered Markdown document offers a `#fragment`.
//!
//! **Role:** derives each heading's anchor the way GitHub does — lowercase, characters other than
//! letters, digits, spaces, hyphens and underscores dropped, spaces turned into hyphens, a repeat
//! of an earlier anchor suffixed `-1`, `-2` in document order — and adds every anchor an `<a id>`
//! or `<a name>` declares.
//!
//! **Position:** reads the headings and explicit anchors of a [`ScannedDocument`]; the link rule
//! ([`super::link_targets`]) matches fragments against the set.
//!
//! **Signals and state:** none; pure functions.
//!
//! **Invariants:** letters and digits of every script survive, other symbols and emoji do not;
//! each space becomes its own hyphen, so an anchor keeps the hyphens a dropped character leaves
//! behind; the anchor set holds every heading of the document, since the scan never reports a
//! heading from inside code.

use std::collections::BTreeSet;

use super::markdown_scan::ScannedDocument;

/// Every anchor `document` offers: its heading slugs, repeats numbered, and its explicit anchors.
pub(super) fn document_anchors(document: &ScannedDocument) -> BTreeSet<String> {
    let mut anchors: BTreeSet<String> = BTreeSet::new();
    let mut repeats: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for heading in &document.headings {
        let base = slug(&heading.text);
        let mut anchor = base.clone();
        while anchors.contains(&anchor) {
            let count = repeats.entry(base.clone()).or_insert(0);
            *count += 1;
            anchor = format!("{base}-{count}");
        }
        anchors.insert(anchor);
    }
    anchors.extend(document.explicit_anchors.iter().cloned());
    anchors
}

/// A heading's anchor: lowercase, only letters, digits, spaces, hyphens and underscores kept,
/// each space a hyphen.
pub(super) fn slug(heading: &str) -> String {
    heading
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

#[cfg(test)]
#[path = "tests/heading_anchors.rs"]
mod tests;
