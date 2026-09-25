//! Patterns — the typed form of an extended regex, a literal needle and a case fold.
//!
//! **Role:** [`Pattern`], a compiled regex or escaped literal whose `^` and `$` anchor lines.
//!
//! **Position:** built by the gates and matched in [`crate::gate`] and [`crate::scan`].
//!
//! **Signals and state:** none; a pattern is compiled once.
//!
//! **Invariants:** the matcher is compiled in, so no search binary can be missing; a match never
//! spans two files.
//!
//! ── WHY THE MATCHER IS COMPILED IN ───────────────────────────────────────────────────────────
//!
//! A gate that shells out to a search binary answers differently depending on who invokes it:
//! a search binary may be missing, differ in version, or be shadowed by a shell function. With
//! the `regex` crate the search engine is inside the gate binary, so there is no `PATH`, no shell
//! function, no version skew, and **exit 127 is not a reachable state for the matcher at all**.
//! The dependency is removed rather than asserted.
//!
//! ── LINE SEMANTICS: `^` AND `$` ──────────────────────────────────────────────────────────────
//!
//! Gate patterns are written the way an operator writes them for a line-oriented search: `^foo`
//! means "some line begins with foo". The `regex` crate defaults to matching the whole text,
//! where `^` means "the input begins with foo" — so the same pattern would quietly match only the
//! first line, and a ban built on `^\s*unsafe` would report OK over a file full of violations.
//!
//! That is the defect this crate exists to prevent, arriving through the back door of a syntax
//! difference, so `multi_line(true)` is set unconditionally and is not configurable. With it,
//! `^`/`$` are line anchors and `.` still does not cross a newline.
//!
//! The rest of the extended-regex surface the gates use — `\(`, `\[`, `|` and POSIX classes such
//! as `[[:space:]]` — needs no translation. A fixed needle is [`Pattern::literal`], which escapes
//! it rather than trusting the caller to have escaped it.

use regex::{Regex, RegexBuilder};

/// A compiled search pattern with line-oriented semantics.
#[derive(Debug, Clone)]
pub struct Pattern {
    re: Regex,
    /// The pattern exactly as the caller wrote it, for failure messages. The compiled form is
    /// unhelpful in a log when the source was a literal needle and got escaped.
    source: String,
}

impl Pattern {
    /// An extended-regex pattern — the default engine.
    pub fn regex(pat: &str) -> Result<Pattern, regex::Error> {
        Ok(Pattern {
            re: build(pat, false)?,
            source: pat.to_string(),
        })
    }

    /// A literal pattern. The needle is escaped, so regex metacharacters in it are matched as
    /// themselves.
    pub fn literal(pat: &str) -> Pattern {
        // `regex::escape` output is always a valid pattern, so this cannot fail.
        let re = build(&regex::escape(pat), false).expect("escaped literal is always valid");
        Pattern {
            re,
            source: pat.to_string(),
        }
    }

    /// Case-fold this pattern.
    pub fn case_insensitive(self) -> Result<Pattern, regex::Error> {
        Ok(Pattern {
            re: build(self.re.as_str(), true)?,
            source: self.source,
        })
    }

    /// Does this pattern match anywhere in `subject`?
    pub fn is_match(&self, subject: &str) -> bool {
        self.re.is_match(subject)
    }

    /// The pattern as the caller wrote it, for diagnostics.
    pub fn source(&self) -> &str {
        &self.source
    }
}

fn build(pat: &str, case_insensitive: bool) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pat)
        // See the module docs. Non-negotiable: without it, `^`/`$` silently change meaning.
        .multi_line(true)
        .case_insensitive(case_insensitive)
        .build()
}

#[cfg(test)]
#[path = "tests/pattern_tests.rs"]
mod tests;
