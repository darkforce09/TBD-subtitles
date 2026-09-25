//! Where a link destination points: this checkout or the web.
//!
//! **Role:** classifies a destination, resolves a checkout path against the tracked tree (from the
//! repository root when it starts with `/`, from the linking document's folder otherwise, with
//! percent-decoding and `.`/`..` normalised), reads the `#L<n>` and `#L<n>-L<m>` line anchors,
//! and says what a fragment asks of the file it is written on.
//!
//! **Position:** called by the link rule ([`super::link_targets`]) for every destination; the
//! tracked files and folders come from [`TrackedTree`].
//!
//! **Signals and state:** none; pure functions.
//!
//! **Invariants:** a destination with a URI scheme (letters, digits, `+` and `-`, then `:`) or
//! starting `//` is off the checkout and never fetched; a path that climbs above the repository
//! root escapes it; only tracked files and folders holding a tracked file exist.

use crate::gate_run::path_regions::{is_markdown, parent_folder};
use crate::gate_run::tracked_tree::TrackedTree;

/// What a destination names.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Destination {
    /// `#fragment` alone: the linking document itself.
    SameDocument { fragment: String },
    /// A path in this checkout, still percent-encoded.
    CheckoutPath {
        path: String,
        /// Whether the query asks GitHub for the plain view (`?plain=1`), where a Markdown file
        /// takes line anchors instead of heading anchors.
        plain_view: bool,
        fragment: Option<String>,
    },
    /// Another host or scheme: counted, never fetched.
    External,
}

/// What a fragment asks of the file it is written on.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum FragmentNeed {
    /// A heading or explicit anchor of a rendered Markdown file.
    Anchor(String),
    /// A line range of a file shown as text.
    Lines(usize, usize),
    /// Neither: a non-Markdown file takes line anchors only.
    Unmatchable(String),
}

/// A checkout path a destination resolves to.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Resolution {
    File(String),
    Folder(String),
    /// Nothing tracked sits at the normalised path.
    Missing(String),
    /// The path climbs above the repository root.
    EscapesRepository,
}

/// Classify a destination as the scan found it.
pub(super) fn classify(destination: &str) -> Destination {
    if let Some(fragment) = destination.strip_prefix('#') {
        return Destination::SameDocument {
            fragment: fragment.to_string(),
        };
    }
    let web = destination.starts_with("//");
    if web || has_scheme(destination) {
        return Destination::External;
    }
    let (rest, fragment) = split_fragment(destination);
    let (path, query) = rest
        .split_once('?')
        .map_or((rest, None), |(path, query)| (path, Some(query)));
    Destination::CheckoutPath {
        path: path.to_string(),
        plain_view: query.is_some_and(asks_plain_view),
        fragment,
    }
}

/// Resolve a still-encoded checkout path written in `document` against the tracked tree.
pub(super) fn resolve(document: &str, path: &str, tree: &TrackedTree) -> Resolution {
    let Some(decoded) = percent_decode(path) else {
        return Resolution::Missing(path.to_string());
    };
    let Some(normalised) = normalise(document, &decoded) else {
        return Resolution::EscapesRepository;
    };
    if tree.is_file(&normalised) {
        Resolution::File(normalised)
    } else if tree.is_folder(&normalised) {
        Resolution::Folder(normalised)
    } else {
        Resolution::Missing(normalised)
    }
}

/// The repository-relative path `decoded` names from `document`'s folder, or from the repository
/// root when it starts with `/`; `None` when `..` climbs above the root.
pub(super) fn normalise(document: &str, decoded: &str) -> Option<String> {
    let base = if decoded.starts_with('/') {
        ""
    } else {
        parent_folder(document)
    };
    let mut parts: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();
    for part in decoded.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    Some(parts.join("/"))
}

/// `text` with every `%XX` escape decoded; a `%` not followed by two hex digits stays as written.
/// `None` when the decoded bytes are not UTF-8.
pub(super) fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escape = (bytes[index] == b'%')
            .then(|| bytes.get(index + 1..index + 3))
            .flatten()
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escape {
            Some(byte) => {
                decoded.push(byte);
                index += 3;
            }
            None => {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).ok()
}

/// The lines a `#L<n>` or `#L<n>-L<m>` fragment selects, or `None` for any other fragment.
pub(super) fn line_anchor(fragment: &str) -> Option<(usize, usize)> {
    let (first, last) = match fragment.split_once('-') {
        Some((first, last)) => (first, Some(last)),
        None => (fragment, None),
    };
    let number = |part: &str| {
        part.strip_prefix('L')
            .filter(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
            .and_then(|digits| digits.parse::<usize>().ok())
    };
    let first = number(first)?;
    let last = match last {
        Some(last) => number(last)?,
        None => first,
    };
    Some((first, last))
}

/// How many lines a viewer numbers in `text`: every line break ends one, and a last line without
/// a break still counts.
pub(super) fn line_count(text: &str) -> usize {
    let breaks = text.matches('\n').count();
    if text.is_empty() || text.ends_with('\n') {
        breaks
    } else {
        breaks + 1
    }
}

/// Why a line anchor does not fit a file of `lines` lines, or `None` when it does.
pub(super) fn line_anchor_problem(first: usize, last: usize, lines: usize) -> Option<String> {
    if first == 0 {
        Some("line numbers start at 1".to_string())
    } else if last < first {
        Some(format!(
            "the range ends at {last} before it starts at {first}"
        ))
    } else if last > lines {
        Some(format!("the file has {lines} line(s)"))
    } else {
        None
    }
}

/// What `fragment` asks of `file`: a heading anchor when GitHub renders the file as Markdown, a
/// line range otherwise.
pub(super) fn fragment_need(file: &str, plain_view: bool, fragment: &str) -> FragmentNeed {
    let anchor = decode_fragment(fragment);
    if is_markdown(file) && !plain_view {
        return FragmentNeed::Anchor(anchor);
    }
    match line_anchor(&anchor) {
        Some((first, last)) => FragmentNeed::Lines(first, last),
        None => FragmentNeed::Unmatchable(anchor),
    }
}

/// Why `anchor` matches nothing in `file`, which GitHub does not render as Markdown.
pub(super) fn unmatchable_anchor_problem(file: &str, anchor: &str) -> String {
    format!(
        "`{file}` is not rendered Markdown, so `{anchor}` matches nothing; only a #L<n> or \
         #L<n>-L<m> line anchor applies"
    )
}

/// A fragment percent-decoded, or as written when it does not decode.
pub(super) fn decode_fragment(fragment: &str) -> String {
    percent_decode(fragment).unwrap_or_else(|| fragment.to_string())
}

/// `(before, fragment)` for a destination; an empty fragment is none.
pub(super) fn split_fragment(destination: &str) -> (&str, Option<String>) {
    match destination.split_once('#') {
        Some((before, fragment)) if !fragment.is_empty() => (before, Some(fragment.to_string())),
        Some((before, _)) => (before, None),
        None => (destination, None),
    }
}

/// Whether `destination` starts with a URI scheme: a letter, then letters, digits, `+` or `-`,
/// then `:`. A dot is left out, so a file name such as `main.rs:12` reads as a path.
fn has_scheme(destination: &str) -> bool {
    let Some((scheme, _)) = destination.split_once(':') else {
        return false;
    };
    scheme
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-')
}

/// Whether a query string holds `plain=1`.
fn asks_plain_view(query: &str) -> bool {
    query.split('&').any(|pair| pair == "plain=1")
}

#[cfg(test)]
#[path = "tests/target_resolution.rs"]
mod tests;
