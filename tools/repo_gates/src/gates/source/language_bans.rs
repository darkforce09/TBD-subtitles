//! The language ban: every tracked file is Rust, Markdown, TOML or JSON.
//!
//! **Role:** the `language-bans` gate. A tracked file passes only when its kind is on the allow
//! list — Rust sources, Markdown, TOML, JSON, `Cargo.lock`, the git and editor configuration
//! files, and subtitle fixtures inside a `tests/fixtures/` folder — and its first line is no
//! shebang naming a shell, Python or Node interpreter.
//!
//! **Position:** a [`FileRule`] run by [`crate::gates::run`]; judges every tracked file.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** the rule is an allow list, so a new script kind is banned before anyone names
//! it; the shebang is parsed, so `#![allow(...)]` in Rust is not one; there is no exemption list.

use crate::gate_run::file_rule::{FileRule, extension};
use crate::gate_run::path_regions::file_name;

/// Extensions every tracked file may carry.
const ALLOWED_EXTENSIONS: [&str; 4] = ["rs", "md", "toml", "json"];

/// Exact file names allowed besides the extensions: the lock file and tool configuration.
const ALLOWED_NAMES: [&str; 4] = [
    "Cargo.lock",
    ".gitignore",
    ".gitattributes",
    ".editorconfig",
];

/// JSON files that make a folder a Node package.
const NODE_MANIFESTS: [&str; 2] = ["package.json", "package-lock.json"];

/// Subtitle formats a test may keep as fixtures.
const FIXTURE_EXTENSIONS: [&str; 3] = ["srt", "vtt", "ass"];

/// Interpreters a shebang may not name.
const BANNED_INTERPRETERS: [&str; 12] = [
    "sh", "bash", "dash", "zsh", "ksh", "fish", "python", "python2", "python3", "node", "deno",
    "bun",
];

/// The `language-bans` gate.
pub(crate) struct LanguageBans;

impl FileRule for LanguageBans {
    fn gate(&self) -> &'static str {
        "language-bans"
    }

    fn describes(&self) -> String {
        "every tracked file is Rust, Markdown, TOML or JSON; no shell, Python, Node or Makefile"
            .to_string()
    }

    fn selects(&self, _path: &str) -> bool {
        true
    }

    fn problems(&self, path: &str, bytes: &[u8]) -> Vec<String> {
        let mut problems = Vec::new();
        if !is_allowed_kind(path) {
            problems.push(format!(
                "{path}: not Rust, Markdown, TOML or JSON; repository tasks are Rust programs \
                 under tools/"
            ));
        }
        let first_line = bytes.split(|byte| *byte == b'\n').next().unwrap_or(&[]);
        if let Some(interpreter) = shebang_interpreter(&String::from_utf8_lossy(first_line))
            && BANNED_INTERPRETERS.contains(&interpreter)
        {
            problems.push(format!("{path}:1: shebang names `{interpreter}`"));
        }
        problems
    }
}

/// Whether `path` is a kind of file the repository may track.
fn is_allowed_kind(path: &str) -> bool {
    let name = file_name(path);
    if NODE_MANIFESTS.contains(&name) {
        return false;
    }
    if ALLOWED_NAMES.contains(&name) {
        return true;
    }
    match extension(path) {
        Some(found) if ALLOWED_EXTENSIONS.contains(&found) => true,
        Some(found) if FIXTURE_EXTENSIONS.contains(&found) => path.contains("/tests/fixtures/"),
        _ => false,
    }
}

/// The program a `#!` first line runs, following `env` and skipping its flags; `None` when the
/// line is no shebang. `#![allow(...)]` yields `[allow(...)]`, which names no interpreter.
pub(crate) fn shebang_interpreter(first_line: &str) -> Option<&str> {
    fn base(token: &str) -> &str {
        token.rsplit('/').next().unwrap_or(token)
    }
    let rest = first_line.strip_prefix("#!")?;
    let mut tokens = rest.split_whitespace();
    let interpreter = base(tokens.next()?);
    if interpreter == "env" {
        return tokens.find(|token| !token.starts_with('-')).map(base);
    }
    Some(interpreter)
}

#[cfg(test)]
#[path = "tests/language_bans.rs"]
mod tests;
