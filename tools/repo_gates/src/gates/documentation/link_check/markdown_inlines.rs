//! The inline pass of the Markdown scan: links, references, code spans and anchors in one run.
//!
//! **Role:** reads one paragraph or heading, lines joined, and finds every inline link and image,
//! every autolink, every full or collapsed reference whose label is undefined, every code span and
//! every `<a id|name>` anchor; it also renders the run's text the way a heading's anchor is
//! derived from it: code span content kept, link destinations and images dropped, HTML tags and
//! matched `_` emphasis delimiters removed.
//!
//! **Position:** called by the block pass ([`super::markdown_scan`]) once per run, with the
//! document's defined labels; destinations and labels are read by [`super::link_destination`].
//!
//! **Signals and state:** none held between runs; one [`InlineScan`] per run.
//!
//! **Invariants:** CommonMark's precedence: a backslash escape, a code span, an autolink, an HTML
//! comment or an HTML tag binds before brackets do, so nothing inside them opens a link; a link
//! may not contain another link, though it may contain an image; a shortcut reference whose label
//! is undefined is plain text, and so is a link with empty text before an undefined label
//! (`[][label]`), which shows nothing to follow; every other full or collapsed reference whose
//! label is undefined is reported, an image with empty alt text (`![][label]`) included, since the
//! image still shows; footnote labels (`^…`) are never references.

use std::collections::BTreeSet;

use super::inline_html::html_tag;
use super::link_destination::{inline_tail, is_escapable, reference_label};
use super::markdown_scan::{CodeSpan, ScannedLink, UndefinedReference};

/// The longest URI scheme an autolink may carry.
const LONGEST_SCHEME: usize = 32;

/// What the inline pass found in one run.
#[derive(Debug, Default)]
pub(super) struct InlineScan {
    pub(super) links: Vec<ScannedLink>,
    pub(super) undefined_references: Vec<UndefinedReference>,
    /// Every `id` or `name` an `<a>` tag in the run declares.
    pub(super) anchors: Vec<String>,
    pub(super) code_spans: Vec<CodeSpan>,
    /// The run's rendered text, the input of a heading's slug.
    pub(super) rendered: String,
}

/// A `[` or `![` still waiting for its `]`.
#[derive(Debug)]
struct Opener {
    at: usize,
    image: bool,
    active: bool,
    /// The rendered text's length before the opener, where an image's alt text is cut.
    rendered_length: usize,
}

/// A run of `_` in the rendered text and whether it may open or close emphasis.
#[derive(Debug)]
struct Underscores {
    at: usize,
    length: usize,
    can_open: bool,
    can_close: bool,
}

/// The inline pass over one run.
struct Cursor<'a> {
    chars: Vec<char>,
    /// The 1-based source line of every character.
    line_of: Vec<usize>,
    labels: &'a BTreeSet<String>,
    openers: Vec<Opener>,
    underscores: Vec<Underscores>,
    found: InlineScan,
}

impl InlineScan {
    /// Scan one run: `(line number, text)` pairs read as one text joined by line breaks, with
    /// references resolved against `labels`, the document's normalised defined labels.
    pub(super) fn run(lines: &[(usize, String)], labels: &BTreeSet<String>) -> InlineScan {
        let mut chars = Vec::new();
        let mut line_of = Vec::new();
        for (position, (number, text)) in lines.iter().enumerate() {
            if position > 0 {
                chars.push('\n');
                line_of.push(*number);
            }
            chars.extend(text.chars());
            line_of.resize(chars.len(), *number);
        }
        let mut cursor = Cursor {
            chars,
            line_of,
            labels,
            openers: Vec::new(),
            underscores: Vec::new(),
            found: InlineScan::default(),
        };
        cursor.scan();
        cursor.found
    }
}

/// A reference label as definitions and uses are matched: whitespace runs collapsed, lowercase.
pub(super) fn normalise_label(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

impl Cursor<'_> {
    fn scan(&mut self) {
        let mut index = 0;
        while index < self.chars.len() {
            index = match self.chars[index] {
                '\\' => self.escape(index),
                '`' => self.code_span(index),
                '<' => self.angle(index),
                '!' if self.chars.get(index + 1) == Some(&'[') => self.open(index, true),
                '[' => self.open(index, false),
                ']' => self.close(index),
                '&' => self.entity(index),
                '_' => self.underscore_run(index),
                c => {
                    self.found.rendered.push(c);
                    index + 1
                }
            };
        }
        self.remove_emphasis();
    }

    fn escape(&mut self, index: usize) -> usize {
        match self.chars.get(index + 1) {
            Some(&next) if is_escapable(next) || next == '\n' => {
                self.found.rendered.push(next);
                index + 2
            }
            _ => {
                self.found.rendered.push('\\');
                index + 1
            }
        }
    }

    /// A code span: a backtick run closed by the next run of the same length; an unmatched run
    /// is literal backticks.
    fn code_span(&mut self, index: usize) -> usize {
        let width = self.run_length(index, '`');
        let mut search = index + width;
        while search < self.chars.len() {
            if self.chars[search] != '`' {
                search += 1;
                continue;
            }
            let closing = self.run_length(search, '`');
            if closing == width {
                let raw: String = self.chars[index + width..search].iter().collect();
                let text = code_span_text(&raw);
                self.found.rendered.push_str(&text);
                self.found.code_spans.push(CodeSpan {
                    line: self.line_of[index],
                    text,
                });
                return search + width;
            }
            search += closing;
        }
        self.found.rendered.push_str(&"`".repeat(width));
        index + width
    }

    /// `<`: an HTML comment, an autolink, an HTML tag, or a literal `<`.
    fn angle(&mut self, index: usize) -> usize {
        if self.starts_with(index, "<!--")
            && let Some(end) = self.find(index + 4, "-->")
        {
            return end + 3;
        }
        if let Some(close) = self.autolink(index) {
            let destination: String = self.chars[index + 1..close].iter().collect();
            self.found.rendered.push_str(&destination);
            self.found.links.push(ScannedLink {
                line: self.line_of[index + 1],
                destination,
            });
            return close + 1;
        }
        if let Some((close, anchors)) = html_tag(&self.chars, index) {
            self.found.anchors.extend(anchors);
            return close + 1;
        }
        self.found.rendered.push('<');
        index + 1
    }

    /// The index of an autolink's `>`: a scheme of letters, digits, `+`, `.` and `-` starting with
    /// a letter, a `:`, then no whitespace, `<` or `>`.
    fn autolink(&self, index: usize) -> Option<usize> {
        let first = *self.chars.get(index + 1)?;
        if !first.is_ascii_alphabetic() {
            return None;
        }
        let scheme = self.chars[index + 1..]
            .iter()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
            .count();
        if !(2..=LONGEST_SCHEME).contains(&scheme)
            || self.chars.get(index + 1 + scheme) != Some(&':')
        {
            return None;
        }
        let mut close = index + 2 + scheme;
        loop {
            match *self.chars.get(close)? {
                '>' => return Some(close),
                c if c.is_whitespace() || c.is_control() || c == '<' => return None,
                _ => close += 1,
            }
        }
    }

    fn open(&mut self, index: usize, image: bool) -> usize {
        self.openers.push(Opener {
            at: index,
            image,
            active: true,
            rendered_length: self.found.rendered.len(),
        });
        self.found.rendered.push_str(if image { "![" } else { "[" });
        index + if image { 2 } else { 1 }
    }

    /// `]`: an inline link, a full, collapsed or shortcut reference, or a literal bracket.
    fn close(&mut self, index: usize) -> usize {
        let Some(opener) = self.openers.pop().filter(|opener| opener.active) else {
            self.found.rendered.push(']');
            return index + 1;
        };
        let text_start = opener.at + if opener.image { 2 } else { 1 };
        if self.chars.get(index + 1) == Some(&'(')
            && let Some(tail) = inline_tail(&self.chars, index + 1)
        {
            self.found.links.push(ScannedLink {
                line: self.line_of[tail.start],
                destination: tail.destination,
            });
            self.matched(&opener);
            return tail.end + 1;
        }
        if self.chars.get(index + 1) == Some(&'[') {
            if let Some((label, end)) = reference_label(&self.chars, index + 1) {
                // A link with empty text shows nothing to follow, so `[][label]` with an
                // undefined label is literal text, as CommonMark renders it (`points[][2]`): the
                // `]` falls through as a literal and the label's brackets are scanned afresh.
                let plain_text = !opener.image && text_start == index && !self.is_defined(&label);
                if !plain_text {
                    self.reference(&label, index + 2);
                    self.matched(&opener);
                    return end + 1;
                }
            } else if self.chars.get(index + 2) == Some(&']') {
                let label: String = self.chars[text_start..index].iter().collect();
                self.reference(&label, text_start);
                self.matched(&opener);
                return index + 3;
            }
        }
        let label: String = self.chars[text_start..index].iter().collect();
        if self.is_defined(&label) {
            self.matched(&opener);
        } else {
            self.found.rendered.push(']');
        }
        index + 1
    }

    /// Whether a definition in the document carries `label`.
    fn is_defined(&self, label: &str) -> bool {
        self.labels.contains(&normalise_label(label))
    }

    /// Record a full or collapsed reference whose label no definition carries.
    fn reference(&mut self, label: &str, at: usize) {
        if !label.starts_with('^') && !self.is_defined(label) {
            self.found.undefined_references.push(UndefinedReference {
                line: self.line_of[at.min(self.line_of.len() - 1)],
                label: label.to_string(),
            });
        }
    }

    /// A link or image closed: an image's alt text leaves the rendered text, and a link keeps its
    /// text and deactivates every `[` before it, since links do not nest.
    fn matched(&mut self, opener: &Opener) {
        if opener.image {
            self.found.rendered.truncate(opener.rendered_length);
            self.underscores
                .retain(|run| run.at < opener.rendered_length);
        } else {
            self.found.rendered.push(']');
            for earlier in self.openers.iter_mut().filter(|earlier| !earlier.image) {
                earlier.active = false;
            }
        }
    }

    /// A character reference renders as its character; a named one renders as `&`, which no
    /// anchor keeps.
    fn entity(&mut self, index: usize) -> usize {
        let Some(end) = self.chars[index..]
            .iter()
            .take(34)
            .position(|c| *c == ';')
            .map(|offset| index + offset)
        else {
            self.found.rendered.push('&');
            return index + 1;
        };
        let body: String = self.chars[index + 1..end].iter().collect();
        let numeric = match body.strip_prefix('#') {
            Some(hex) if hex.starts_with(['x', 'X']) => u32::from_str_radix(&hex[1..], 16).ok(),
            Some(decimal) => decimal.parse::<u32>().ok(),
            None => None,
        };
        let named = body.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && body.chars().all(|c| c.is_ascii_alphanumeric());
        match (numeric, named) {
            (Some(code), _) => {
                let decoded = char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER);
                self.found.rendered.push(decoded);
                end + 1
            }
            (None, true) => {
                self.found.rendered.push('&');
                end + 1
            }
            (None, false) => {
                self.found.rendered.push('&');
                index + 1
            }
        }
    }

    /// A run of `_`, recorded with whether CommonMark lets it open or close emphasis.
    fn underscore_run(&mut self, index: usize) -> usize {
        let length = self.run_length(index, '_');
        let before = index
            .checked_sub(1)
            .map_or(' ', |previous| self.chars[previous]);
        let after = self.chars.get(index + length).copied().unwrap_or(' ');
        let left = !after.is_whitespace()
            && (!is_punctuation(after) || before.is_whitespace() || is_punctuation(before));
        let right = !before.is_whitespace()
            && (!is_punctuation(before) || after.is_whitespace() || is_punctuation(after));
        self.underscores.push(Underscores {
            at: self.found.rendered.len(),
            length,
            can_open: left && (!right || is_punctuation(before)),
            can_close: right && (!left || is_punctuation(after)),
        });
        self.found.rendered.push_str(&"_".repeat(length));
        index + length
    }

    /// Remove the `_` delimiters that pair up as emphasis from the rendered text; unpaired ones
    /// stay literal.
    fn remove_emphasis(&mut self) {
        let mut remaining: Vec<usize> = self.underscores.iter().map(|run| run.length).collect();
        let mut openers: Vec<usize> = Vec::new();
        let mut removed: Vec<(usize, usize)> = Vec::new();
        for closer in 0..self.underscores.len() {
            if self.underscores[closer].can_close {
                while remaining[closer] > 0 {
                    let Some(&opener) = openers.last() else {
                        break;
                    };
                    let used = remaining[opener].min(remaining[closer]).min(2);
                    let opening = &self.underscores[opener];
                    let closing = &self.underscores[closer];
                    removed.push((opening.at + remaining[opener] - used, used));
                    removed.push((closing.at + closing.length - remaining[closer], used));
                    remaining[opener] -= used;
                    remaining[closer] -= used;
                    if remaining[opener] == 0 {
                        openers.pop();
                    }
                }
            }
            if remaining[closer] > 0 && self.underscores[closer].can_open {
                openers.push(closer);
            }
        }
        removed.sort_unstable();
        let rendered = std::mem::take(&mut self.found.rendered);
        let mut kept = String::with_capacity(rendered.len());
        let mut from = 0;
        for (at, length) in removed {
            kept.push_str(&rendered[from..at]);
            from = at + length;
        }
        kept.push_str(&rendered[from..]);
        self.found.rendered = kept;
    }

    fn run_length(&self, index: usize, marker: char) -> usize {
        self.chars[index..]
            .iter()
            .take_while(|c| **c == marker)
            .count()
    }

    fn starts_with(&self, index: usize, text: &str) -> bool {
        text.chars()
            .enumerate()
            .all(|(offset, c)| self.chars.get(index + offset) == Some(&c))
    }

    /// The index at or after `from` where `text` starts.
    fn find(&self, from: usize, text: &str) -> Option<usize> {
        (from..self.chars.len()).find(|index| self.starts_with(*index, text))
    }
}

/// A code span's content: line breaks as spaces, and one space stripped from each end when both
/// ends hold one and the content is not only spaces.
fn code_span_text(raw: &str) -> String {
    let text = raw.replace('\n', " ");
    let bare = text.len() >= 2
        && text.starts_with(' ')
        && text.ends_with(' ')
        && !text.chars().all(|c| c == ' ');
    if bare {
        text[1..text.len() - 1].to_string()
    } else {
        text
    }
}

/// Whether `c` counts as punctuation for emphasis flanking: ASCII punctuation, or any other
/// character that is neither a letter, a digit nor whitespace.
fn is_punctuation(c: char) -> bool {
    c.is_ascii_punctuation() || (!c.is_ascii() && !c.is_alphanumeric() && !c.is_whitespace())
}

#[cfg(test)]
#[path = "tests/markdown_inlines.rs"]
mod tests;
