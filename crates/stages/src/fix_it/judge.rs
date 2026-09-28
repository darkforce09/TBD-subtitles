//! The last pass: each changed line held against the line before the change, in the brief's
//! context, and accepted or turned down.
//!
//! **Role:** show the judge every changed line (its problems, before and after, each family's
//! reason, the heard words it now leaves out, what the engines heard, the lines around it) in
//! batches, and read one verdict per line.
//!
//! **Position:** called by `fix_it::run` after the repairs; its verdicts decide which changes
//! are kept.
//!
//! **Signals and state:** one call per batch of `BATCH` changed lines, several at once.
//!
//! **Invariants:** only a changed line is judged; a line the answer leaves out, or whose call
//! failed, has no verdict, and so no change; the first verdict of a line counts.

use std::collections::HashMap;

use serde::Deserialize;

use super::calls::Calls;
use super::evidence::{self, Lookup};
use super::{Draft, Episode, guard, prompt};

/// Changed lines per judge call.
pub const BATCH: usize = 40;

/// What the judge made of a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Accept(String),
    TurnDown(String),
}

#[derive(Debug, Deserialize)]
struct Answer {
    id: String,
    accept: bool,
    why: String,
}

#[derive(Debug, Deserialize)]
struct Answers {
    verdicts: Vec<Answer>,
}

/// A verdict for each changed draft the judge answered; `progress` hears `(calls done, calls)`.
pub(crate) fn judge(
    ep: &Episode,
    context: &str,
    drafts: &HashMap<String, Draft>,
    calls: &Calls,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> HashMap<String, Verdict> {
    let mut changed: Vec<&Draft> = drafts.values().filter(|d| d.changed()).collect();
    changed.sort_by_key(|d| d.index);
    let mut verdicts = HashMap::new();
    if changed.is_empty() || calls.stopped() {
        return verdicts;
    }
    let lookup = Lookup::new(ep);
    let batches: Vec<&[&Draft]> = changed.chunks(BATCH).collect();
    let messages: Vec<String> = batches
        .iter()
        .map(|batch| message(context, &lookup, batch))
        .collect();
    let label = "checks of the changes";
    let answers = calls.ask_all(
        label,
        prompt::JUDGE,
        &prompt::judge_schema(),
        &messages,
        progress,
    );
    for (batch, answer) in batches.iter().zip(answers) {
        let Some(answer) = answer.and_then(|value| calls.read::<Answers>(label, value)) else {
            continue;
        };
        for verdict in answer.verdicts {
            let asked = batch.iter().any(|d| d.id == verdict.id);
            if !asked || verdicts.contains_key(&verdict.id) {
                continue;
            }
            let why = verdict.why.trim().to_string();
            let verdict_of = if verdict.accept {
                Verdict::Accept(why)
            } else {
                Verdict::TurnDown(why)
            };
            verdicts.insert(verdict.id, verdict_of);
        }
    }
    verdicts
}

/// The message for one batch: the brief, then each change's block.
fn message(context: &str, lookup: &Lookup, batch: &[&Draft]) -> String {
    let mut text = format!("{context}\nChanges to check:\n");
    for draft in batch {
        text.push('\n');
        text.push_str(&block(lookup, draft));
    }
    text
}

/// What the judge reads about one change.
pub(crate) fn block(lookup: &Lookup, draft: &Draft) -> String {
    let reasons: Vec<String> = draft
        .steps
        .iter()
        .map(|s| format!("{}: {}", s.family.describe(), s.why))
        .collect();
    let removed = guard::removed(&draft.before_text, &draft.text);
    let leaves_out = if removed.is_empty() {
        "nothing".to_string()
    } else {
        removed
            .iter()
            .map(|w| format!("\"{w}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "### {} {}\nProblems: {}\nBefore: {}\nAfter: {}\nWhy: {}\nLeaves out: {leaves_out}\n{}{}",
        draft.id,
        lookup.place(draft.index),
        draft.problems.join("; "),
        evidence::shown(&draft.before_text, &draft.before_flags),
        evidence::shown(&draft.text, &draft.flags),
        reasons.join("; "),
        lookup.heard(draft.index),
        lookup.around(draft.index)
    )
}
