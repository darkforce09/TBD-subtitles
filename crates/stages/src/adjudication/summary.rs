//! A run's answer summed up: the checks' counts, the flags used, lines changed, and how often each
//! glossary name appears before and after.
//!
//! **Role:** turn an adjudication and its findings into flat numbers for reports.
//!
//! **Position:** called by the stack spike tools; later by the job report.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a name is counted as a whole word, case as written.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::Adjudication;
use super::checks::Findings;
use crate::diff_sheet::sheet::Utterance;

/// The summary of `result` against `sheet`, as report entries.
pub fn summarize(
    sheet: &[Utterance],
    result: &Adjudication,
    findings: &Findings,
    glossary: &[&str],
) -> Map<String, Value> {
    let mut flags: BTreeMap<String, usize> = BTreeMap::new();
    for line in &result.lines {
        for f in &line.f {
            *flags.entry(f.clone()).or_default() += 1;
        }
    }
    let before: String = sheet
        .iter()
        .flat_map(|u| &u.words)
        .map(|w| format!("{} ", w.text))
        .collect();
    let after: String = result.lines.iter().map(|l| format!("{} ", l.t)).collect();
    let mut names = BTreeMap::new();
    for name in glossary {
        let (b, a) = (count_word(&before, name), count_word(&after, name));
        if b != a {
            names.insert(name.to_string(), format!("{b} -> {a}"));
        }
    }
    let changed = result
        .lines
        .iter()
        .filter(|l| {
            sheet.iter().find(|u| u.id == l.id).is_some_and(|u| {
                let words: Vec<&str> = u.words.iter().map(|w| w.text.as_str()).collect();
                words.join(" ") != l.t
            })
        })
        .count();
    let mut out = Map::new();
    out.insert("utterances".into(), sheet.len().into());
    out.insert("lines_returned".into(), result.lines.len().into());
    out.insert("changed_lines".into(), changed.into());
    out.insert("calls".into(), result.calls.into());
    out.insert("failed_calls".into(), result.failed_calls.len().into());
    out.insert("input_tokens".into(), result.input_tokens.into());
    out.insert("output_tokens".into(), result.output_tokens.into());
    out.insert("cost_usd".into(), result.cost_usd.into());
    out.insert("missing_ids".into(), findings.missing_ids.len().into());
    out.insert("duplicate_ids".into(), findings.duplicate_ids.len().into());
    out.insert("novel_words".into(), findings.novel.len().into());
    out.insert(
        "removed_locked_words".into(),
        findings.removed_locked.len().into(),
    );
    out.insert("too_fast_lines".into(), findings.too_fast.len().into());
    out.insert("flags".into(), json!(flags));
    out.insert("name_count_changes".into(), json!(names));
    out
}

/// Whole-word occurrences of `name` in `text`.
pub fn count_word(text: &str, name: &str) -> usize {
    text.match_indices(name)
        .filter(|(i, _)| {
            let before = text[..*i].chars().next_back();
            let after = text[i + name.len()..].chars().next();
            !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
        })
        .count()
}

#[cfg(test)]
#[path = "tests/summary.rs"]
mod tests;
