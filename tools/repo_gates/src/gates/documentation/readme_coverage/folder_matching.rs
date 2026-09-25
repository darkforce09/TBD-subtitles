//! Pairing a Contents block's entries with a folder's tracked children.
//!
//! **Role:** checks that every tracked direct child except README.md matches exactly one entry of
//! its own kind, and that every entry matches at least one tracked child.
//!
//! **Position:** called by [`super`] with the entries [`super::contents_block`] read and the
//! children the tracked tree lists for the folder.
//!
//! **Signals and state:** none; pure over its inputs.
//!
//! **Invariants:** a file child is matched only by file entries and a folder child only by folder
//! entries; README.md is never a child; a child no entry matches is reported at the root line,
//! and every other violation at the line of the entry it concerns.

use super::contents_block::ContentsEntry;
use super::{ChildKind, Violation};
use crate::gate_run::path_regions::README;
use crate::gate_run::tracked_tree::FolderChildren;

/// Every violation between `entries` and the tracked `children` of their folder.
pub(super) fn match_entries(
    children: &FolderChildren,
    entries: &[ContentsEntry],
    root_line: usize,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    let mut entry_matched = vec![false; entries.len()];
    for (name, kind) in listed_children(children) {
        let hits: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.kind == kind && entry.pattern.matches(name))
            .map(|(index, _)| index)
            .collect();
        for index in &hits {
            entry_matched[*index] = true;
        }
        let child = shown(name, kind);
        match hits.as_slice() {
            [] => violations.push(Violation {
                line: root_line,
                message: format!("tracked child `{child}` matches no entry"),
            }),
            [_] => {}
            [first, ..] => {
                let lines: Vec<String> = hits
                    .iter()
                    .map(|index| entries[*index].line.to_string())
                    .collect();
                violations.push(Violation {
                    line: entries[*first].line,
                    message: format!(
                        "tracked child `{child}` matches {} entries (lines {}); it must match \
                         exactly one",
                        hits.len(),
                        lines.join(", ")
                    ),
                });
            }
        }
    }
    for (entry, matched) in entries.iter().zip(entry_matched) {
        if !matched {
            violations.push(Violation {
                line: entry.line,
                message: unmatched_entry(entry, children),
            });
        }
    }
    violations
}

/// The children a Contents block lists: every tracked file but README.md, then every folder.
fn listed_children(children: &FolderChildren) -> impl Iterator<Item = (&str, ChildKind)> {
    children
        .files
        .iter()
        .map(String::as_str)
        .filter(|name| *name != README)
        .map(|name| (name, ChildKind::File))
        .chain(
            children
                .folders
                .iter()
                .map(|name| (name.as_str(), ChildKind::Folder)),
        )
}

/// Why an entry matches no child, as precisely as the folder can say.
fn unmatched_entry(entry: &ContentsEntry, children: &FolderChildren) -> String {
    let token = &entry.token;
    if entry.kind == ChildKind::File && entry.pattern.is_name(README) {
        return format!("entry `{token}` lists the README itself, which Contents leaves out");
    }
    let crossed = listed_children(children)
        .find(|(name, kind)| *kind != entry.kind && entry.pattern.matches(name));
    match crossed {
        Some((name, ChildKind::Folder)) => format!(
            "entry `{token}` is a file entry, but `{name}/` is a folder; a folder entry ends in `/`"
        ),
        Some((name, ChildKind::File)) => format!(
            "entry `{token}` is a folder entry, but `{name}` is a file; only a folder entry ends \
             in `/`"
        ),
        None => format!("entry `{token}` matches no tracked child"),
    }
}

/// A child as a Contents entry would spell it: a folder with its trailing `/`.
fn shown(name: &str, kind: ChildKind) -> String {
    match kind {
        ChildKind::File => name.to_string(),
        ChildKind::Folder => format!("{name}/"),
    }
}

#[cfg(test)]
#[path = "tests/folder_matching.rs"]
mod tests;
