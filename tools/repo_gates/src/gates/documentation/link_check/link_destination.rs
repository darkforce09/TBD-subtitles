//! Link destinations, titles and reference labels, character by character.
//!
//! **Role:** reads the parts of a link the rules resolve: an inline link's `(destination
//! "title")` tail, a reference definition line `[label]: destination "title"`, and a bracketed
//! reference label.
//!
//! **Position:** called by the block pass ([`super::markdown_scan`]) for definitions and by the
//! inline pass ([`super::markdown_inlines`]) for inline links and references.
//!
//! **Signals and state:** none; pure functions over characters.
//!
//! **Invariants:** CommonMark's destination rules: an angle destination `<…>` may hold spaces but
//! no line break or unescaped `<`; a bare destination holds no space or control character and only
//! balanced parentheses, at most [`DEEPEST_PARENTHESES`] deep; a backslash before ASCII punctuation
//! yields the punctuation; a title is separated from its destination by whitespace.

/// The deepest parenthesis nesting a bare destination may hold.
const DEEPEST_PARENTHESES: usize = 32;

/// The longest reference label, in characters.
const LONGEST_LABEL: usize = 999;

/// A reference definition line: the label as written and the destination it defines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Definition {
    pub(super) label: String,
    pub(super) destination: String,
}

/// An inline link's tail: the destination and where it sits.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct InlineTail {
    /// The destination, escapes resolved, angle brackets removed.
    pub(super) destination: String,
    /// The index of the destination's first character, or of the closing `)` when it is empty.
    pub(super) start: usize,
    /// The index of the closing `)`.
    pub(super) end: usize,
}

/// The tail of an inline link whose `(` sits at `open`, or `None` when the characters there do
/// not form one.
pub(super) fn inline_tail(chars: &[char], open: usize) -> Option<InlineTail> {
    let start = skip_whitespace(chars, open + 1, true);
    let (destination, after) = destination(chars, start)?;
    let mut at = skip_whitespace(chars, after, true);
    if (at > after || destination.is_empty())
        && let Some(end) = title(chars, at)
    {
        at = skip_whitespace(chars, end, true);
    }
    (chars.get(at) == Some(&')')).then_some(InlineTail {
        destination,
        start,
        end: at,
    })
}

/// The reference definition a whole line (indentation removed) makes, or `None`. A footnote
/// definition (`[^label]:`) is not a reference definition.
pub(super) fn definition(line: &str) -> Option<Definition> {
    let chars: Vec<char> = line.chars().collect();
    if chars.first() != Some(&'[') {
        return None;
    }
    let (label, close) = reference_label(&chars, 0)?;
    if label.starts_with('^') || chars.get(close + 1) != Some(&':') {
        return None;
    }
    let start = skip_whitespace(&chars, close + 2, false);
    let (destination, after) = destination(&chars, start)?;
    if destination.is_empty() && chars.get(start) != Some(&'<') {
        return None;
    }
    let mut at = skip_whitespace(&chars, after, false);
    if at > after
        && let Some(end) = title(&chars, at)
    {
        at = skip_whitespace(&chars, end, false);
    }
    (at == chars.len()).then_some(Definition { label, destination })
}

/// The label of the bracket pair opening at `open`, as written, and the index of its `]`; `None`
/// when the brackets do not close, nest, run past [`LONGEST_LABEL`] or hold only whitespace.
pub(super) fn reference_label(chars: &[char], open: usize) -> Option<(String, usize)> {
    let mut label = String::new();
    let mut index = open + 1;
    loop {
        match *chars.get(index)? {
            ']' => break,
            '[' => return None,
            '\\' if chars.get(index + 1).is_some_and(|next| is_escapable(*next)) => {
                label.push('\\');
                label.push(chars[index + 1]);
                index += 2;
            }
            c => {
                label.push(c);
                index += 1;
            }
        }
        if label.chars().count() > LONGEST_LABEL {
            return None;
        }
    }
    (!label.trim().is_empty()).then_some((label, index))
}

/// Whether a backslash before `c` escapes it.
pub(super) fn is_escapable(c: char) -> bool {
    c.is_ascii_punctuation()
}

/// The destination starting at `at` and the index just past it.
fn destination(chars: &[char], at: usize) -> Option<(String, usize)> {
    let mut text = String::new();
    let mut index = at;
    if chars.get(at) == Some(&'<') {
        index += 1;
        loop {
            match *chars.get(index)? {
                '>' => return Some((text, index + 1)),
                '<' | '\n' => return None,
                '\\' if chars.get(index + 1).is_some_and(|next| is_escapable(*next)) => {
                    text.push(chars[index + 1]);
                    index += 2;
                }
                c => {
                    text.push(c);
                    index += 1;
                }
            }
        }
    }
    let mut depth = 0usize;
    while let Some(&c) = chars.get(index) {
        if c == '\\' && chars.get(index + 1).is_some_and(|next| is_escapable(*next)) {
            text.push(chars[index + 1]);
            index += 2;
            continue;
        }
        if c == ' ' || c.is_ascii_control() {
            break;
        }
        if c == '(' {
            depth += 1;
            if depth > DEEPEST_PARENTHESES {
                return None;
            }
        } else if c == ')' {
            if depth == 0 {
                break;
            }
            depth -= 1;
        }
        text.push(c);
        index += 1;
    }
    (depth == 0).then_some((text, index))
}

/// The index just past a title opening at `at`, or `None` when none opens or it never closes.
fn title(chars: &[char], at: usize) -> Option<usize> {
    let close = match chars.get(at)? {
        '"' => '"',
        '\'' => '\'',
        '(' => ')',
        _ => return None,
    };
    let mut index = at + 1;
    while let Some(&c) = chars.get(index) {
        if c == '\\' && chars.get(index + 1).is_some_and(|next| is_escapable(*next)) {
            index += 2;
            continue;
        }
        if c == close {
            return Some(index + 1);
        }
        if close == ')' && c == '(' {
            return None;
        }
        index += 1;
    }
    None
}

/// The index past the spaces and tabs at `at`, and past one line break too when `across_lines`.
fn skip_whitespace(chars: &[char], at: usize, across_lines: bool) -> usize {
    let mut index = at;
    let mut crossed = false;
    while let Some(&c) = chars.get(index) {
        match c {
            ' ' | '\t' => {}
            '\n' if across_lines && !crossed => crossed = true,
            _ => break,
        }
        index += 1;
    }
    index
}

#[cfg(test)]
#[path = "tests/link_destination.rs"]
mod tests;
