//! The six assertion helpers every gate is written against.
//!
//! **Role:** `ban` and `require` over files, their `_str` forms over text in memory, and the
//! `probe` pair for compound conditions.
//!
//! **Position:** called by the gates; reads files and matches with [`crate::pattern`].
//!
//! **Signals and state:** reads the target files; holds nothing.
//!
//! **Invariants:** a target that cannot be read is `DidNotRun`, never a quiet pass.
//!
//! | over files     | over a subject in memory | compound conditions |
//! |----------------|--------------------------|---------------------|
//! | [`ban`]        | [`ban_str`]              | [`probe_files`]     |
//! | [`require`]    | [`require_str`]          | [`probe_str`]       |
//!
//! [`ban`] and [`require`] answer with a [`Verdict`], so a target that could not be read is a
//! `DidNotRun` and never a quiet pass. The `_str` pair takes text the caller already holds, where
//! no `NotRun` is reachable, and still answers in the same vocabulary so gates read alike.
//!
//! ── WHY `probe` RETURNS A `Result`, NOT A `bool` ─────────────────────────────────────────────
//!
//! A compound condition — "A appears AND B does not" — needs the raw answer rather than a
//! verdict, and neither outcome is a failure on its own. Were the answer a bare `bool`, an input
//! that could not be read would answer `false`, and the whole condition would short-circuit to
//! clean: the fail-open shape in a costume.
//!
//! [`probe_str`] and [`probe_files`] return `Result<bool, NotRun>`, so `?` propagates "did not
//! run" and a caller cannot reach the boolean without having handled the other case.

use std::path::Path;

use crate::pattern::Pattern;
use crate::verdict::{Kind, NotRun, Verdict};

/// Read every target, failing closed on the first one that is missing or unreadable.
///
/// Presence and content are one step because the classification needs the bytes, and "I could not
/// open it" must never reach a comparison as an empty string.
fn read_all(files: &[&Path]) -> Result<String, NotRun> {
    let mut joined = String::new();
    for f in files {
        // `is_file()` first: a directory or a dangling symlink would otherwise surface as an
        // opaque io::Error, and "the path is not a file" is the far more common real cause.
        if !f.is_file() {
            return Err(NotRun::TargetMissing(f.to_path_buf()));
        }
        match std::fs::read_to_string(f) {
            Ok(text) => {
                joined.push_str(&text);
                // Guarantee a boundary so a pattern cannot match across two files' contents:
                // a match must name one file an operator can open and fix.
                if !joined.ends_with('\n') {
                    joined.push('\n');
                }
            }
            Err(source) => {
                return Err(NotRun::Unreadable {
                    path: f.to_path_buf(),
                    source,
                });
            }
        }
    }
    Ok(joined)
}

/// `pattern` must NOT appear in `files`.
pub fn ban(msg: &str, pattern: &Pattern, files: &[&Path]) -> Verdict {
    match read_all(files) {
        Err(cause) => Verdict::did_not_run(msg, Kind::Ban, cause),
        Ok(text) if pattern.is_match(&text) => Verdict::failed(msg),
        // No match — and, crucially, we know it holds *because the search ran*.
        Ok(_) => Verdict::Held,
    }
}

/// `pattern` MUST appear in `files`.
pub fn require(msg: &str, pattern: &Pattern, files: &[&Path]) -> Verdict {
    match read_all(files) {
        Err(cause) => Verdict::did_not_run(msg, Kind::Pin, cause),
        Ok(text) if pattern.is_match(&text) => Verdict::Held,
        Ok(_) => Verdict::failed(msg),
    }
}

/// `pattern` must NOT appear in an in-memory subject.
///
/// No `NotRun` path exists: the subject is already in hand, so there is nothing that could fail
/// to be examined. Returning a `Verdict` anyway keeps every gate in one vocabulary.
pub fn ban_str(msg: &str, pattern: &Pattern, subject: &str) -> Verdict {
    if pattern.is_match(subject) {
        Verdict::failed(msg)
    } else {
        Verdict::Held
    }
}

/// `pattern` MUST appear in an in-memory subject.
pub fn require_str(msg: &str, pattern: &Pattern, subject: &str) -> Verdict {
    if pattern.is_match(subject) {
        Verdict::Held
    } else {
        Verdict::failed(msg)
    }
}

/// Compound-condition escape hatch over an in-memory subject.
///
/// Infallible today, but returns `Result` for symmetry with [`probe_files`] so that a caller
/// written against one can be switched to the other without restructuring its `?`s.
pub fn probe_str(pattern: &Pattern, subject: &str) -> Result<bool, NotRun> {
    Ok(pattern.is_match(subject))
}

/// Compound-condition escape hatch over files. `?` propagates "did not run".
pub fn probe_files(pattern: &Pattern, files: &[&Path]) -> Result<bool, NotRun> {
    Ok(pattern.is_match(&read_all(files)?))
}

#[cfg(test)]
#[path = "tests/gate_tests.rs"]
mod tests;
