//! Contents entry names and globs.
//!
//! **Role:** turns an entry's name into a matcher: the exact name, or a glob of `*`, `?`, `[…]`
//! and `{a,b}` compiled to an anchored regular expression.
//!
//! **Position:** built by [`super::contents_block`] for every well-formed entry; asked by
//! [`super::folder_matching`] whether a child's name matches.
//!
//! **Signals and state:** none; a pattern is immutable once compiled.
//!
//! **Invariants:** a name without glob characters matches only itself; a glob matches whole names
//! only, and its wildcards match any character, a leading dot included; a malformed glob is
//! refused with its reason, never compiled into a looser pattern.

use regex::Regex;

/// The characters that make an entry a glob rather than a name.
const GLOB_CHARACTERS: [char; 4] = ['*', '?', '[', '{'];

/// How a Contents entry matches child names.
#[derive(Debug)]
pub(super) enum EntryPattern {
    /// Matches exactly this name.
    Name(String),
    /// Matches every whole name the compiled glob accepts.
    Glob(Regex),
}

impl EntryPattern {
    /// The matcher for `name`, or why it is not a valid glob, phrased to follow the entry.
    pub(super) fn parse(name: &str) -> Result<EntryPattern, String> {
        if !name.contains(GLOB_CHARACTERS) {
            return Ok(EntryPattern::Name(name.to_string()));
        }
        let expression = glob_expression(name)?;
        Regex::new(&expression)
            .map(EntryPattern::Glob)
            .map_err(|error| format!("is not a valid glob: {error}"))
    }

    /// Whether `candidate` matches.
    pub(super) fn matches(&self, candidate: &str) -> bool {
        match self {
            EntryPattern::Name(name) => name == candidate,
            EntryPattern::Glob(glob) => glob.is_match(candidate),
        }
    }

    /// Whether the pattern is exactly the name `candidate`, not a glob that happens to match it.
    pub(super) fn is_name(&self, candidate: &str) -> bool {
        matches!(self, EntryPattern::Name(name) if name == candidate)
    }
}

/// The anchored regular expression a glob compiles to: `*` any run of characters, `?` one
/// character, `[…]` one character of a set, `{a,b}` either alternative (alternatives nest and
/// may hold globs); every other character matches itself.
fn glob_expression(glob: &str) -> Result<String, String> {
    let characters: Vec<char> = glob.chars().collect();
    let mut expression = String::from("^(?:");
    let mut open_braces = 0usize;
    let mut index = 0;
    while index < characters.len() {
        match characters[index] {
            '*' => expression.push_str(".*"),
            '?' => expression.push('.'),
            '[' => {
                index = character_class(&characters, index, &mut expression)?;
                continue;
            }
            '{' => {
                open_braces += 1;
                expression.push_str("(?:");
            }
            ',' if open_braces > 0 => expression.push('|'),
            '}' if open_braces > 0 => {
                open_braces -= 1;
                expression.push(')');
            }
            literal => expression.push_str(&regex::escape(&literal.to_string())),
        }
        index += 1;
    }
    if open_braces > 0 {
        return Err("has an unclosed `{`".to_string());
    }
    expression.push_str(")$");
    Ok(expression)
}

/// Append the character class that opens at `characters[open]`, a `[`, and return the index just
/// past its closing `]`. A leading `!` or `^` negates the class, a `]` in first place is a member,
/// and `a-z` is a range.
fn character_class(
    characters: &[char],
    open: usize,
    expression: &mut String,
) -> Result<usize, String> {
    let mut index = open + 1;
    expression.push('[');
    if matches!(characters.get(index), Some('!' | '^')) {
        expression.push('^');
        index += 1;
    }
    let first_member = index;
    loop {
        let Some(&member) = characters.get(index) else {
            return Err("has an unclosed `[`".to_string());
        };
        if member == ']' && index > first_member {
            expression.push(']');
            return Ok(index + 1);
        }
        expression.push_str(&regex::escape(&member.to_string()));
        match (characters.get(index + 1), characters.get(index + 2)) {
            (Some('-'), Some(&last)) if last != ']' => {
                expression.push('-');
                expression.push_str(&regex::escape(&last.to_string()));
                index += 3;
            }
            _ => index += 1,
        }
    }
}

#[cfg(test)]
#[path = "tests/entry_pattern.rs"]
mod tests;
