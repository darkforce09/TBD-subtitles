//! The file-length limit: production Rust files stay under 500 lines, test files under 1000.
//!
//! **Role:** the `file-length` gate over every tracked `.rs` file; a file inside a `tests`
//! folder is a test file.
//!
//! **Position:** a [`FileRule`] run by [`crate::gates::run`].
//!
//! **Signals and state:** none.
//!
//! **Invariants:** no exemption list; a longer file splits by responsibility.

use crate::gate_run::file_rule::{FileRule, extension, is_test_path};

/// A production file must hold fewer lines than this.
pub(crate) const PRODUCTION_LIMIT: usize = 500;

/// A test file must hold fewer lines than this.
pub(crate) const TEST_LIMIT: usize = 1000;

/// The `file-length` gate.
pub(crate) struct FileLength;

impl FileRule for FileLength {
    fn gate(&self) -> &'static str {
        "file-length"
    }

    fn describes(&self) -> String {
        format!(
            "production Rust files hold fewer than {PRODUCTION_LIMIT} lines, test files fewer \
             than {TEST_LIMIT}"
        )
    }

    fn selects(&self, path: &str) -> bool {
        extension(path) == Some("rs")
    }

    fn problems(&self, path: &str, bytes: &[u8]) -> Vec<String> {
        let (limit, kind) = if is_test_path(path) {
            (TEST_LIMIT, "test")
        } else {
            (PRODUCTION_LIMIT, "production")
        };
        let lines = String::from_utf8_lossy(bytes).lines().count();
        if lines < limit {
            Vec::new()
        } else {
            vec![format!(
                "{path}: {lines} lines; a {kind} file holds fewer than {limit}, so split it by \
                 responsibility"
            )]
        }
    }
}

#[cfg(test)]
#[path = "tests/file_length.rs"]
mod tests;
