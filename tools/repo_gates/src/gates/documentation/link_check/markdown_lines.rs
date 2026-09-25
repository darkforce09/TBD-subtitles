//! The shape of a single Markdown line: which block, if any, it opens.
//!
//! **Role:** recognises the line-level markers the block pass needs: list item markers, ATX
//! headings, setext underlines, thematic breaks, blockquote markers, the front matter block, and
//! leading indentation with its tabs expanded.
//!
//! **Position:** called by the block pass in [`super::markdown_scan`]; fences come from
//! [`crate::gate_run::markdown_fences`].
//!
//! **Signals and state:** none; pure functions over one line, or the leading lines of a document.
//!
//! **Invariants:** CommonMark's line rules: a block opens at most [`DEEPEST_BLOCK_INDENT`] spaces
//! past its container, indented code starts at [`CODE_INDENT`], a tab advances to the next
//! multiple of [`TAB_STOP`] columns, and a list item's content starts one to four spaces past its
//! marker.

use crate::gate_run::markdown_fences;

/// The deepest indentation, relative to its container, at which a line still opens a block.
pub(super) const DEEPEST_BLOCK_INDENT: usize = 3;

/// The indentation, relative to its container, at which a line becomes indented code.
pub(super) const CODE_INDENT: usize = 4;

/// The columns a tab advances to the next multiple of.
const TAB_STOP: usize = 4;

/// A list item marker: how far its content starts past the marker's first character, and
/// whether it may interrupt a paragraph (a bullet, or an ordered item numbered 1).
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ListItem {
    pub(super) content_offset: usize,
    pub(super) interrupts_paragraph: bool,
}

/// The list item `line` (its indentation removed) opens, if any.
pub(super) fn list_item(line: &str) -> Option<ListItem> {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    let (marker, interrupts) = match line.chars().next()? {
        '-' | '+' | '*' => (1, true),
        _ if (1..=9).contains(&digits)
            && matches!(line[digits..].chars().next(), Some('.' | ')')) =>
        {
            (digits + 1, line[..digits].parse::<u64>().ok() == Some(1))
        }
        _ => return None,
    };
    let after = &line[marker..];
    let spaces = after.chars().take_while(|c| *c == ' ').count();
    if after.is_empty() {
        return Some(ListItem {
            content_offset: marker,
            interrupts_paragraph: false,
        });
    }
    if spaces == 0 {
        return None;
    }
    let gap = if spaces > CODE_INDENT { 1 } else { spaces };
    Some(ListItem {
        content_offset: marker + gap,
        interrupts_paragraph: interrupts,
    })
}

/// The text of an ATX heading line, its closing `#` sequence removed.
pub(super) fn atx_heading(trimmed: &str) -> Option<&str> {
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &trimmed[level..];
    if !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
        return None;
    }
    let text = rest.trim();
    let without_closing = text.trim_end_matches('#');
    if without_closing.is_empty() {
        return Some("");
    }
    if without_closing.ends_with([' ', '\t']) {
        return Some(without_closing.trim_end());
    }
    Some(text)
}

/// Whether a trimmed line opens a block of its own rather than continuing a paragraph: a list
/// item that may interrupt one, a thematic break or setext underline, an ATX heading, a fence or
/// an HTML comment.
pub(super) fn opens_block(trimmed: &str) -> bool {
    list_item(trimmed).is_some_and(|item| item.interrupts_paragraph)
        || is_thematic_break(trimmed)
        || is_setext_underline(trimmed)
        || atx_heading(trimmed).is_some()
        || markdown_fences::opening(trimmed).is_some()
        || trimmed.starts_with("<!--")
}

/// Whether a trimmed line is a setext underline: only `=` or only `-`.
pub(super) fn is_setext_underline(trimmed: &str) -> bool {
    !trimmed.is_empty() && (trimmed.chars().all(|c| c == '=') || trimmed.chars().all(|c| c == '-'))
}

/// Whether a line is a thematic break: three or more `*`, `-` or `_`, spaces allowed between.
pub(super) fn is_thematic_break(trimmed: &str) -> bool {
    let Some(marker) = trimmed.chars().next() else {
        return false;
    };
    matches!(marker, '*' | '-' | '_')
        && trimmed
            .chars()
            .all(|c| c == marker || c == ' ' || c == '\t')
        && trimmed.chars().filter(|c| *c == marker).count() >= 3
}

/// The index of the first line after a leading `---` front matter block, or 0 when the document
/// opens with none; a block that never closes is not front matter.
pub(super) fn front_matter_end(lines: &[&str]) -> usize {
    if lines.first().map(|line| line.trim_end()) != Some("---") {
        return 0;
    }
    lines
        .iter()
        .skip(1)
        .position(|line| matches!(line.trim_end(), "---" | "..."))
        .map_or(0, |closing| closing + 2)
}

/// The line without its leading blockquote markers, each with its one optional space.
pub(super) fn strip_blockquote(line: &str) -> &str {
    let mut rest = line;
    loop {
        let indent = leading_spaces(rest);
        match rest[indent..].strip_prefix('>') {
            Some(quoted) if indent <= DEEPEST_BLOCK_INDENT => {
                rest = quoted.strip_prefix(' ').unwrap_or(quoted);
            }
            _ => return rest,
        }
    }
}

/// The line with the tabs of its leading whitespace expanded to spaces, so indentation is a count
/// of leading bytes.
pub(super) fn expand_leading_tabs(line: &str) -> String {
    let whitespace = line.len() - line.trim_start_matches([' ', '\t']).len();
    let mut expanded = String::with_capacity(line.len());
    for c in line[..whitespace].chars() {
        if c == '\t' {
            let width = TAB_STOP - expanded.len() % TAB_STOP;
            expanded.push_str(&" ".repeat(width));
        } else {
            expanded.push(c);
        }
    }
    expanded.push_str(&line[whitespace..]);
    expanded
}

/// How many spaces a line starts with.
pub(super) fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

#[cfg(test)]
#[path = "tests/markdown_lines.rs"]
mod tests;
