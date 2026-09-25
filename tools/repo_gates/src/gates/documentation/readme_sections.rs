//! README sections: every README holds the core sections, in order, and nothing else at `##`.
//!
//! **Role:** the `readme-sections` gate over every README.md in the README span. The title is an
//! H1 and the first heading; a README in a code tree carries no status line; every `##` heading
//! is one the README standard names, in the standard's order and at most once; `## Contents` and
//! `## Boundaries` are present; and Boundaries holds exactly three top-level bullets, spelled
//! `Depends on:`, `Used by:` and `Rules:`, in that order.
//!
//! **Position:** a [`FileRule`] run by [`crate::gates::run`]; the Contents block itself is
//! judged by [`super::readme_coverage`]; headings inside fenced blocks are skipped through
//! [`crate::gate_run::markdown_fences`].
//!
//! **Signals and state:** none.
//!
//! **Invariants:** which kind sections a folder needs stays with the writer; the gate checks
//! only that a heading is known and in order.

use crate::gate_run::file_rule::FileRule;
use crate::gate_run::markdown_fences::{self, Fence};
use crate::gate_run::path_regions::{
    README, file_name, in_code_tree, in_readme_span, parent_folder,
};

/// Every `##` heading a README may hold, in the order they must appear.
pub(crate) const SECTION_ORDER: [&str; 11] = [
    "Contents",
    "How it works",
    "Getting started",
    "Configuration",
    "Public surface",
    "Commands",
    "Format",
    "Producers and consumers",
    "Code",
    "Boundaries",
    "Related documentation",
];

/// The sections every README holds.
const REQUIRED: [&str; 2] = ["Contents", "Boundaries"];

/// The three bullets of Boundaries, in order.
const BOUNDARY_BULLETS: [&str; 3] = ["Depends on:", "Used by:", "Rules:"];

/// The status line's opening, which a code README never carries.
const STATUS_PREFIX: &str = "**Status:**";

/// The `readme-sections` gate.
pub(crate) struct ReadmeSections;

impl FileRule for ReadmeSections {
    fn gate(&self) -> &'static str {
        "readme-sections"
    }

    fn describes(&self) -> String {
        "every README holds the core sections in order and Boundaries has the three bullets"
            .to_string()
    }

    fn selects(&self, path: &str) -> bool {
        file_name(path) == README && in_readme_span(parent_folder(path))
    }

    fn problems(&self, path: &str, bytes: &[u8]) -> Vec<String> {
        let text = String::from_utf8_lossy(bytes);
        let headings = headings(&text);
        let mut problems = Vec::new();
        if in_code_tree(path) && text.starts_with(STATUS_PREFIX) {
            problems.push(format!("{path}:1: a code README carries no status line"));
        }
        match headings.first() {
            Some((_, 1, _)) => {}
            Some((line, _, _)) => {
                problems.push(format!("{path}:{line}: the first heading is the H1 title"))
            }
            None => problems.push(format!("{path}:1: no H1 title")),
        }
        let mut last: Option<usize> = None;
        for (line, _, title) in headings.iter().filter(|(_, level, _)| *level == 2) {
            match SECTION_ORDER.iter().position(|known| known == title) {
                None => problems.push(format!(
                    "{path}:{line}: `## {title}` is no README section; finer structure goes in `###`"
                )),
                Some(rank) if last.is_some_and(|before| rank <= before) => problems.push(format!(
                    "{path}:{line}: `## {title}` is out of order or repeated"
                )),
                Some(rank) => last = Some(rank),
            }
        }
        for required in REQUIRED {
            if !headings
                .iter()
                .any(|(_, level, title)| *level == 2 && title == required)
            {
                problems.push(format!("{path}:1: no `## {required}` section"));
            }
        }
        problems.extend(boundary_problems(path, &text));
        problems
    }
}

/// Every ATX heading outside fenced blocks: line number, level and title.
fn headings(text: &str) -> Vec<(usize, usize, String)> {
    let mut open: Option<Fence> = None;
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if let Some(fence) = open {
            if markdown_fences::closes(fence, line) {
                open = None;
            }
            continue;
        }
        if let Some((fence, _)) = markdown_fences::opening(line) {
            open = Some(fence);
            continue;
        }
        let level = line.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&level) && line[level..].starts_with(' ') {
            found.push((index + 1, level, line[level..].trim().to_string()));
        }
    }
    found
}

/// The problems of the Boundaries section: exactly the three bullets, in order.
fn boundary_problems(path: &str, text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines.iter().position(|line| *line == "## Boundaries") else {
        return Vec::new();
    };
    let bullets: Vec<(usize, &str)> = lines[start + 1..]
        .iter()
        .enumerate()
        .take_while(|(_, line)| !line.starts_with("## "))
        .filter_map(|(offset, line)| {
            line.strip_prefix("- ")
                .map(|rest| (start + offset + 2, rest))
        })
        .collect();
    let spelled: Vec<bool> = BOUNDARY_BULLETS
        .iter()
        .zip(&bullets)
        .map(|(label, (_, bullet))| bullet.starts_with(label))
        .collect();
    if bullets.len() == BOUNDARY_BULLETS.len() && spelled.iter().all(|ok| *ok) {
        return Vec::new();
    }
    vec![format!(
        "{path}:{}: Boundaries holds exactly three bullets, `- Depends on:`, `- Used by:` and \
         `- Rules:`, in that order; found {}",
        start + 1,
        bullets.len()
    )]
}

#[cfg(test)]
#[path = "tests/readme_sections.rs"]
mod tests;
