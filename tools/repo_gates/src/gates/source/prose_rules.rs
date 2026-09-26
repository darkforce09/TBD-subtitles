//! The prose rules: code and live documents speak in the present tense, with no tracking ids.
//!
//! **Role:** the `prose-rules` gate over production Rust files, every README.md, the live
//! documents under the documentation root and the project instructions. It fails on a ticket id
//! anywhere, on a history word (a word that tells the past of the code) outside the
//! decision log, and on a milestone id in Rust files and code READMEs, whose words describe the
//! code as it is.
//!
//! **Position:** a [`FileRule`] run by [`crate::gates::run`]; frozen records and test files are
//! not judged.
//!
//! **Signals and state:** none; the patterns compile once per rule.
//!
//! **Invariants:** the history words are matched as whole words in any letter case; the words
//! are spelled in two halves here, so this file never flags itself.

use regex::Regex;

use crate::gate_run::file_rule::{FileRule, extension, is_test_path};
use crate::gate_run::path_regions::{
    README, file_name, in_code_tree, in_documentation_root, is_frozen_record, is_markdown,
};
use crate::layout::PROJECT_INSTRUCTIONS;

/// The decision log's folder, whose files record how choices changed and so may use the history
/// words.
const DECISION_LOG: &str = "documentation/decisions/";

/// Words that describe code or documents by their past, each split so this file never matches.
const HISTORY_WORDS: [(&str, &str); 8] = [
    ("form", "erly"),
    ("previ", "ously"),
    ("used to ", "be"),
    ("leg", "acy"),
    ("ported ", "from"),
    ("migrated ", "(?:from|to)"),
    ("renamed ", "from"),
    ("rewritten ", "from"),
];

/// The `prose-rules` gate.
pub(crate) struct ProseRules {
    ticket_id: Regex,
    milestone_id: Regex,
    history_word: Regex,
}

impl ProseRules {
    pub(crate) fn new() -> ProseRules {
        let words: Vec<String> = HISTORY_WORDS
            .iter()
            .map(|(head, tail)| format!("{head}{tail}"))
            .collect();
        ProseRules {
            ticket_id: Regex::new(r"\bT-[0-9]{2,4}(\.[0-9]+)*\b").expect("a valid pattern"),
            milestone_id: Regex::new(r"\bM[0-9](\.[0-9])?\b").expect("a valid pattern"),
            history_word: Regex::new(&format!(r"(?i)\b(?:{})\b", words.join("|")))
                .expect("a valid pattern"),
        }
    }
}

impl FileRule for ProseRules {
    fn gate(&self) -> &'static str {
        "prose-rules"
    }

    fn describes(&self) -> String {
        "no ticket ids, no history words outside the decision log, no milestone ids in code or \
         code READMEs"
            .to_string()
    }

    fn selects(&self, path: &str) -> bool {
        if is_test_path(path) || is_frozen_record(path) {
            return false;
        }
        extension(path) == Some("rs")
            || file_name(path) == README
            || path == PROJECT_INSTRUCTIONS
            || (in_documentation_root(path) && is_markdown(path))
    }

    fn problems(&self, path: &str, bytes: &[u8]) -> Vec<String> {
        let text = String::from_utf8_lossy(bytes);
        let judge_history = !path.starts_with(DECISION_LOG);
        let judge_milestones =
            extension(path) == Some("rs") || (in_code_tree(path) && file_name(path) == README);
        let mut problems = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let at = index + 1;
            if let Some(found) = self.ticket_id.find(line) {
                problems.push(format!("{path}:{at}: ticket id `{}`", found.as_str()));
            }
            if judge_milestones && let Some(found) = self.milestone_id.find(line) {
                problems.push(format!(
                    "{path}:{at}: milestone id `{}`; the roadmap tracks milestones",
                    found.as_str()
                ));
            }
            if judge_history && let Some(found) = self.history_word.find(line) {
                problems.push(format!(
                    "{path}:{at}: history word `{}`; say what is, and leave the past to git",
                    found.as_str()
                ));
            }
        }
        problems
    }
}

#[cfg(test)]
#[path = "tests/prose_rules.rs"]
mod tests;
