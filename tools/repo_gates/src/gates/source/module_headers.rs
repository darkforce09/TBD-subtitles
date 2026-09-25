//! The module header: crate roots and long files open with the four-part `//!` header.
//!
//! **Role:** the `module-headers` gate. Every production `lib.rs` and `main.rs`, and every
//! production `.rs` file of [`LONG_FILE`] lines or more, opens with a `//!` block that names, in
//! this order, **Role:**, **Position:**, **Signals and state:** and **Invariants:**.
//!
//! **Position:** a [`FileRule`] run by [`crate::gates::run`].
//!
//! **Signals and state:** none.
//!
//! **Invariants:** only the leading `//!` block counts, so a label in a later comment does not
//! stand in for the header; test files are not judged.

use crate::gate_run::file_rule::{FileRule, extension, is_test_path};
use crate::gate_run::path_regions::file_name;

/// A production file of this many lines or more is non-trivial and needs the header.
pub(crate) const LONG_FILE: usize = 80;

/// The labels the header names, in order.
pub(crate) const LABELS: [&str; 4] = [
    "**Role:**",
    "**Position:**",
    "**Signals and state:**",
    "**Invariants:**",
];

/// The `module-headers` gate.
pub(crate) struct ModuleHeaders;

impl FileRule for ModuleHeaders {
    fn gate(&self) -> &'static str {
        "module-headers"
    }

    fn describes(&self) -> String {
        format!(
            "crate roots and production files of {LONG_FILE} lines or more open with the Role, \
             Position, Signals and state, Invariants header"
        )
    }

    fn selects(&self, path: &str) -> bool {
        extension(path) == Some("rs") && !is_test_path(path)
    }

    fn problems(&self, path: &str, bytes: &[u8]) -> Vec<String> {
        let text = String::from_utf8_lossy(bytes);
        let root = matches!(file_name(path), "lib.rs" | "main.rs");
        let lines = text.lines().count();
        if !root && lines < LONG_FILE {
            return Vec::new();
        }
        let header: String = text
            .lines()
            .map_while(|line| line.strip_prefix("//!"))
            .collect::<Vec<_>>()
            .join("\n");
        let why = if root {
            "a crate root".to_string()
        } else {
            format!("{lines} lines")
        };
        let mut from = 0;
        let mut problems = Vec::new();
        for label in LABELS {
            match header[from..].find(label) {
                Some(at) => from += at + label.len(),
                None => problems.push(format!(
                    "{path}: {why}, but the leading //! header does not name {label} after the \
                     labels before it"
                )),
            }
        }
        problems
    }
}

#[cfg(test)]
#[path = "tests/module_headers.rs"]
mod tests;
