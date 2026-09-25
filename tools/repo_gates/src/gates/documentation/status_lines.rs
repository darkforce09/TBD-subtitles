//! Status lines: every document under the documentation root says whether it is live.
//!
//! **Role:** the `status-lines` gate. Every Markdown file under the documentation root opens with
//! `**Status:** live`, `**Status:** frozen record (YYYY-MM-DD)` or `**Status:** archived`; a
//! document other than a README.md inside a frozen folder is a frozen record or archived, never
//! live.
//!
//! **Position:** a [`FileRule`] run by [`crate::gates::run`]; the frozen folders come from
//! [`crate::layout`].
//!
//! **Signals and state:** none; the pattern compiles once per rule.
//!
//! **Invariants:** the status line is the first line, exactly; a date is a real `YYYY-MM-DD`
//! shape, not checked against a calendar.

use regex::Regex;

use crate::gate_run::file_rule::FileRule;
use crate::gate_run::path_regions::{in_documentation_root, is_frozen_record, is_markdown};

/// The `status-lines` gate.
pub(crate) struct StatusLines {
    status: Regex,
}

impl StatusLines {
    pub(crate) fn new() -> StatusLines {
        StatusLines {
            status: Regex::new(
                r"^\*\*Status:\*\* (live|frozen record \([0-9]{4}-[0-9]{2}-[0-9]{2}\)|archived)$",
            )
            .expect("a valid pattern"),
        }
    }
}

impl FileRule for StatusLines {
    fn gate(&self) -> &'static str {
        "status-lines"
    }

    fn describes(&self) -> String {
        "every document under the documentation root opens with its status line".to_string()
    }

    fn selects(&self, path: &str) -> bool {
        in_documentation_root(path) && is_markdown(path)
    }

    fn problems(&self, path: &str, bytes: &[u8]) -> Vec<String> {
        let text = String::from_utf8_lossy(bytes);
        let first = text.lines().next().unwrap_or("");
        let Some(found) = self.status.captures(first) else {
            return vec![format!(
                "{path}:1: the first line is `**Status:** live`, `**Status:** frozen record \
                 (YYYY-MM-DD)` or `**Status:** archived`"
            )];
        };
        if is_frozen_record(path) && &found[1] == "live" {
            return vec![format!(
                "{path}:1: a document in a frozen folder is a frozen record or archived"
            )];
        }
        Vec::new()
    }
}

#[cfg(test)]
#[path = "tests/status_lines.rs"]
mod tests;
