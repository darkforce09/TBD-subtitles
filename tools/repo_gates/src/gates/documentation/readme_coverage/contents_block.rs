//! The Contents block of a README: where it sits and what its lines say.
//!
//! **Role:** finds the first `text` code block after the `## Contents` heading and before the
//! next `## ` heading, checks its root line against the folder, skips every blank or spacer line,
//! and reads every other line into a [`ContentsEntry`] or a [`Violation`].
//!
//! **Position:** called by [`super`] with a README's text and its folder; the entries it returns
//! go on to [`super::folder_matching`].
//!
//! **Signals and state:** none; pure over the README text.
//!
//! **Invariants:** a heading or block inside another fenced block is never taken for the Contents
//! heading or block; every violation names the 1-based README line it concerns; an entry whose
//! token cannot name a direct child never reaches matching.

use super::entry_pattern::EntryPattern;
use super::{ChildKind, Violation};
use crate::gate_run::markdown_fences::{self, Fence};

/// The heading the Contents block follows.
const CONTENTS_HEADING: &str = "## Contents";

/// How a heading that ends the Contents section begins.
const SECTION_HEADING: &str = "## ";

/// The info string on the Contents block's opening fence.
const CONTENTS_INFO_STRING: &str = "text";

/// The tree drawing's branch to an entry that has a sibling after it.
const BRANCH: &str = "├── ";

/// The tree drawing's branch to the last entry of a list.
const LAST_BRANCH: &str = "└── ";

/// The tree drawing's continuation of an outer branch; only a nested entry carries it.
const CONTINUATION: &str = "│   ";

/// The most spaces that may indent a direct-child line that carries no branch.
const DEEPEST_PLAIN_INDENT: usize = 4;

/// What separates an entry from its role: two spaces, and any more that follow.
const ROLE_SEPARATOR: &str = "  ";

/// One entry line: the name or glob of the direct children it lists, and their kind.
#[derive(Debug)]
pub(super) struct ContentsEntry {
    /// The 1-based README line the entry sits on.
    pub(super) line: usize,
    /// The entry as written, its trailing `/` included.
    pub(super) token: String,
    pub(super) kind: ChildKind,
    pub(super) pattern: EntryPattern,
}

/// A located Contents block, read against its folder.
#[derive(Debug)]
pub(super) struct ContentsBlock {
    /// The README line of the root line, where a child that matches no entry is reported.
    pub(super) root_line: usize,
    pub(super) entries: Vec<ContentsEntry>,
    /// Grammar violations of the root line and the entry lines.
    pub(super) violations: Vec<Violation>,
}

/// Locate the Contents block of `readme` and read it against `folder`, or the one violation that
/// says why there is no block to read.
pub(super) fn read_contents(readme: &str, folder: &str) -> Result<ContentsBlock, Violation> {
    let lines: Vec<&str> = readme.lines().collect();
    let mut open_fence: Option<Fence> = None;
    let mut heading: Option<usize> = None;
    for (index, line) in lines.iter().enumerate() {
        if let Some(fence) = open_fence {
            if markdown_fences::closes(fence, line) {
                open_fence = None;
            }
            continue;
        }
        if let Some((fence, info)) = markdown_fences::opening(line) {
            if heading.is_some() && info == CONTENTS_INFO_STRING {
                return read_block(&lines, index, fence, folder);
            }
            open_fence = Some(fence);
            continue;
        }
        if line.starts_with(SECTION_HEADING) {
            match heading {
                Some(at) => return Err(no_block(at)),
                None if line.trim_end() == CONTENTS_HEADING => heading = Some(index),
                None => {}
            }
        }
    }
    Err(heading.map_or_else(
        || Violation {
            line: 1,
            message: format!("no `{CONTENTS_HEADING}` heading"),
        },
        no_block,
    ))
}

/// The violation for a Contents section, headed at 0-based line `heading`, that holds no block.
fn no_block(heading: usize) -> Violation {
    Violation {
        line: heading + 1,
        message: format!(
            "the `{CONTENTS_HEADING}` section holds no ```{CONTENTS_INFO_STRING} code block"
        ),
    }
}

/// Read the block whose fence opens at 0-based line `open`.
fn read_block(
    lines: &[&str],
    open: usize,
    fence: Fence,
    folder: &str,
) -> Result<ContentsBlock, Violation> {
    let close = (open + 1..lines.len())
        .find(|index| markdown_fences::closes(fence, lines[*index]))
        .ok_or_else(|| Violation {
            line: open + 1,
            message: "the Contents block never closes".to_string(),
        })?;
    let root = format!("{folder}/");
    let mut block = ContentsBlock {
        root_line: open + 2,
        entries: Vec::new(),
        violations: Vec::new(),
    };
    let Some((root_text, entry_lines)) = lines[open + 1..close].split_first() else {
        block.root_line = open + 1;
        block.violations.push(Violation {
            line: open + 1,
            message: format!(
                "the Contents block is empty; its first line is the root line `{root}`"
            ),
        });
        return Ok(block);
    };
    if root_text.trim_end() != root {
        block.violations.push(Violation {
            line: block.root_line,
            message: format!(
                "the root line is `{}`, not the folder path `{root}`",
                root_text.trim_end()
            ),
        });
    }
    for (offset, text) in entry_lines.iter().enumerate() {
        if lists_nothing(text) {
            continue;
        }
        if let Some(entry) = read_entry(block.root_line + 1 + offset, text, &mut block.violations) {
            block.entries.push(entry);
        }
    }
    Ok(block)
}

/// Read one entry line, one that is neither blank nor a spacer, pushing every violation it
/// carries. The entry comes back whenever its token can name a direct child, so a missing role is
/// reported without also reporting the child it lists as unlisted.
fn read_entry(line: usize, text: &str, violations: &mut Vec<Violation>) -> Option<ContentsEntry> {
    let mut fault = |message: String| violations.push(Violation { line, message });
    let (prefix, rest) = split_prefix(text);
    let (token, role) = match rest.split_once(ROLE_SEPARATOR) {
        Some((token, role)) => (token, role.trim()),
        None => (rest.split(' ').next().unwrap_or(rest), ""),
    };
    if !is_direct_prefix(prefix) {
        fault(format!(
            "entry `{token}` sits deeper than a direct child; Contents lists direct children only"
        ));
        return None;
    }
    if role.is_empty() {
        fault(format!(
            "entry `{token}` has no role; two or more spaces separate an entry from its role"
        ));
    }
    let (name, kind) = match token.strip_suffix('/') {
        Some(name) => (name, ChildKind::Folder),
        None => (token, ChildKind::File),
    };
    if name.is_empty() {
        fault(format!("entry `{token}` names nothing"));
        return None;
    }
    if name.contains('/') {
        fault(format!(
            "entry `{token}` holds a `/` inside it; Contents lists direct children only"
        ));
        return None;
    }
    match EntryPattern::parse(name) {
        Ok(pattern) => Some(ContentsEntry {
            line,
            token: token.to_string(),
            kind,
            pattern,
        }),
        Err(reason) => {
            fault(format!("entry `{token}` {reason}"));
            None
        }
    }
}

/// Split a line into its tree-drawing prefix (the leading run of branches, continuations and
/// single spaces) and the rest.
fn split_prefix(text: &str) -> (&str, &str) {
    let mut rest = text;
    while let Some(after) = [BRANCH, LAST_BRANCH, CONTINUATION, " "]
        .iter()
        .find_map(|unit| rest.strip_prefix(unit))
    {
        rest = after;
    }
    text.split_at(text.len() - rest.len())
}

/// Whether a prefix sets its entry at the depth of a direct child: no prefix, one branch, or at
/// most [`DEEPEST_PLAIN_INDENT`] spaces. A continuation, a second branch or deeper indentation
/// sets it below another entry.
fn is_direct_prefix(prefix: &str) -> bool {
    prefix == BRANCH
        || prefix == LAST_BRANCH
        || (prefix.len() <= DEEPEST_PLAIN_INDENT && prefix.bytes().all(|byte| byte == b' '))
}

/// Whether a Contents line lists nothing: it holds only whitespace and tree-drawing characters, so
/// it is blank or a spacer such as `│` or `│   │`. A line that holds anything else keeps it past
/// [`split_prefix`], which strips only tree drawing and spaces, so [`read_entry`] always has a
/// token to read.
fn lists_nothing(text: &str) -> bool {
    text.chars()
        .all(|character| character.is_whitespace() || is_tree_drawing(character))
}

/// Whether `character` is one of the box-drawing characters the tree drawing is made of.
fn is_tree_drawing(character: char) -> bool {
    matches!(character, '├' | '└' | '│' | '─')
}

#[cfg(test)]
#[path = "tests/contents_block.rs"]
mod tests;
