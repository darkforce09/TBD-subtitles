//! Fenced code blocks in a Markdown document.
//!
//! **Role:** recognises the lines that open and close a fenced code block, so a scanner can tell
//! a heading or a Contents block from text that only sits inside another code block.
//!
//! **Position:** a line-level helper; the Contents parser in [`super::readme_coverage`] carries
//! the open [`Fence`] from one line to the next.
//!
//! **Signals and state:** none; the caller owns the open fence.
//!
//! **Invariants:** CommonMark's fence rules: at most three spaces of indentation, then a run of at
//! least three backticks or three tildes, then the info string, which never holds a backtick on a
//! backtick fence; a fence closes on a run of the same character at least as long, with nothing
//! after it but whitespace.

/// The fewest fence characters that open a block.
const SHORTEST_FENCE: usize = 3;

/// The most spaces a fence line may be indented by.
const DEEPEST_FENCE_INDENT: usize = 3;

/// An open fenced block: the character its fence repeats and how many times.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Fence {
    marker: char,
    width: usize,
}

/// The fence `line` opens and its trimmed info string, or `None` when the line opens no fence.
pub(crate) fn opening(line: &str) -> Option<(Fence, &str)> {
    let body = without_indent(line)?;
    let marker = body.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let width = body.chars().take_while(|c| *c == marker).count();
    if width < SHORTEST_FENCE {
        return None;
    }
    // Both fence characters are one byte wide, so `width` is also a byte offset.
    let info = body[width..].trim();
    if marker == '`' && info.contains('`') {
        return None;
    }
    Some((Fence { marker, width }, info))
}

/// Whether `line` closes the block `fence` opened.
pub(crate) fn closes(fence: Fence, line: &str) -> bool {
    let Some(body) = without_indent(line) else {
        return false;
    };
    let width = body.chars().take_while(|c| *c == fence.marker).count();
    width >= fence.width && body[width..].trim().is_empty()
}

/// The line without its leading spaces, when there are at most [`DEEPEST_FENCE_INDENT`] of them.
fn without_indent(line: &str) -> Option<&str> {
    let body = line.trim_start_matches(' ');
    (line.len() - body.len() <= DEEPEST_FENCE_INDENT).then_some(body)
}

#[cfg(test)]
#[path = "tests/markdown_fences.rs"]
mod tests;
