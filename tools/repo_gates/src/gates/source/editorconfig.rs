//! The whitespace rules of `.editorconfig`, checked over every tracked text file.
//!
//! **Role:** the `editorconfig` gate. Every tracked file is UTF-8, ends its lines with LF alone,
//! ends with a newline when it is not empty, and carries no trailing whitespace; Markdown may end
//! a line in whitespace, since two trailing spaces are a hard break there.
//!
//! **Position:** a [`FileRule`] run by [`crate::gates::run`]; the rules mirror the repository's
//! `.editorconfig`.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a file is judged byte for byte, so a stray carriage return or a byte that is
//! not UTF-8 is found wherever it sits; each problem names its first line only once per kind.

use crate::gate_run::file_rule::FileRule;
use crate::gate_run::path_regions::is_markdown;

/// The `editorconfig` gate.
pub(crate) struct EditorConfig;

impl FileRule for EditorConfig {
    fn gate(&self) -> &'static str {
        "editorconfig"
    }

    fn describes(&self) -> String {
        "UTF-8, LF line ends, a final newline, no trailing whitespace outside Markdown".to_string()
    }

    fn selects(&self, _path: &str) -> bool {
        true
    }

    fn problems(&self, path: &str, bytes: &[u8]) -> Vec<String> {
        let mut problems = Vec::new();
        let text = match std::str::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => {
                let line = bytes[..error.valid_up_to()]
                    .iter()
                    .filter(|byte| **byte == b'\n')
                    .count()
                    + 1;
                return vec![format!("{path}:{line}: not UTF-8")];
            }
        };
        let first_line_with =
            |found: &dyn Fn(&str) -> bool| text.split('\n').position(found).map(|index| index + 1);
        if let Some(line) = first_line_with(&|line: &str| line.contains('\r')) {
            problems.push(format!(
                "{path}:{line}: carriage return; lines end with LF alone"
            ));
        }
        if !is_markdown(path)
            && let Some(line) =
                first_line_with(&|line: &str| line.trim_end_matches('\r').ends_with([' ', '\t']))
        {
            problems.push(format!("{path}:{line}: trailing whitespace"));
        }
        if !text.is_empty() && !text.ends_with('\n') {
            let line = text.split('\n').count();
            problems.push(format!("{path}:{line}: no newline at the end of the file"));
        }
        problems
    }
}

#[cfg(test)]
#[path = "tests/editorconfig.rs"]
mod tests;
