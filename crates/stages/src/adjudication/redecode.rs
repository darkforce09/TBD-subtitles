//! The re-decode of unsure utterances: which spans to hear again, how the new hypotheses reach
//! the sheet, and the second pass that asks the language model about those ids only.
//!
//! **Role:** after the first pass, list the utterances flagged `UNSURE`, give their padded spans
//! to both engines again on the vocal stem, add what they heard to the sheet as extra hypotheses,
//! and ask the model once more with the settled lines around each one as context.
//!
//! **Position:** called by the re-decode and re-adjudicate steps of the pipeline; the engines run
//! in their workers, this module only plans and merges.
//!
//! **Signals and state:** one model call per batch of unsure ids.
//!
//! **Invariants:** only unsure ids are asked again and only their lines are replaced; every word
//! the engines heard again counts as heard for the checks; a line the second pass left out keeps
//! its first answer.

use std::collections::{HashMap, HashSet};

use inference::llm::LanguageModel;
use job_model::outputs::{Line, Redecode, TimeSpan, Utterance};

use super::{Adjudication, BATCH, ask_with, prompt};

/// Seconds heard on either side of an unsure utterance.
pub const PAD_S: f64 = 0.5;

/// What the second pass adds to the rules.
pub const SECOND_PASS: &str = "\n\nSecond pass. Each utterance below was left UNSURE before. It now also shows `ALT` hypotheses: \
the same stretch heard again on the isolated voice track, `p` by the main engine and `w` by the \
second one. Lines marked CONTEXT are settled neighbours for meaning only: do not return them. \
Return only the listed ids. Keep UNSURE only if the hypotheses still do not settle what was said.";

/// The ids the first pass flagged `UNSURE`, in sheet order.
pub fn unsure_ids(lines: &[Line]) -> Vec<String> {
    lines
        .iter()
        .filter(|l| l.has_flag("UNSURE"))
        .map(|l| l.id.clone())
        .collect()
}

/// The padded span of each id, clamped to the video.
pub fn spans(sheet: &[Utterance], ids: &[String], duration_s: f64) -> Vec<TimeSpan> {
    let by_id: HashMap<&str, &Utterance> = sheet.iter().map(|u| (u.id.as_str(), u)).collect();
    ids.iter()
        .filter_map(|id| by_id.get(id.as_str()))
        .map(|u| {
            TimeSpan::new(
                (u.start_s - PAD_S).max(0.0),
                (u.end_s + PAD_S).min(duration_s),
            )
        })
        .collect()
}

/// The sheet with each re-decoded utterance's new hypotheses added, tagged `p` and `w` (or the
/// given tags), and shown on its line as `ALT p: "…" w: "…"`.
pub fn with_alternatives(
    sheet: &[Utterance],
    alternatives: &[(&str, &Redecode)],
) -> Vec<Utterance> {
    let mut heard: HashMap<&str, Vec<(String, Vec<String>)>> = HashMap::new();
    for (tag, redecode) in alternatives {
        for (id, chunk) in redecode.ids.iter().zip(&redecode.transcript.chunks) {
            let words = chunk.words.iter().map(|w| w.text.clone()).collect();
            heard
                .entry(id.as_str())
                .or_default()
                .push((tag.to_string(), words));
        }
    }
    sheet
        .iter()
        .map(|u| {
            let mut u = u.clone();
            if let Some(extra) = heard.get(u.id.as_str()) {
                u.line.push_str(" ALT");
                for (tag, words) in extra {
                    u.line.push_str(&format!(" {tag}: \"{}\"", words.join(" ")));
                }
                u.hypotheses.extend(extra.iter().cloned());
            }
            u
        })
        .collect()
}

/// The second-pass message: for each id, its settled neighbours as context and its sheet line.
pub fn user_message(
    glossary: &[&str],
    sheet: &[Utterance],
    first: &[Line],
    ids: &[String],
) -> String {
    let text: HashMap<&str, &str> = first
        .iter()
        .map(|l| (l.id.as_str(), l.t.as_str()))
        .collect();
    let asked: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut message = String::from("Glossary (names and terms, spelled right): ");
    message.push_str(&glossary.join(", "));
    message.push_str("\n\nUtterances:\n");
    for (i, u) in sheet.iter().enumerate() {
        if !asked.contains(u.id.as_str()) {
            continue;
        }
        let context = |j: usize| {
            sheet
                .get(j)
                .filter(|n| !asked.contains(n.id.as_str()))
                .and_then(|n| {
                    text.get(n.id.as_str())
                        .map(|t| format!("CONTEXT {}: {t}\n", n.id))
                })
        };
        if let Some(before) = i.checked_sub(1).and_then(context) {
            message.push_str(&before);
        }
        message.push_str(&u.line);
        message.push('\n');
        if let Some(after) = context(i + 1) {
            message.push_str(&after);
        }
    }
    message
}

/// Ask `model` about `ids` only, over the sheet with alternatives; ids an answer left out are
/// asked once more. Returns the second-pass lines of those ids. `progress` hears `(batches asked,
/// batches)`; a batch asked again counts once.
pub fn readjudicate(
    model: &mut dyn LanguageModel,
    sheet: &[Utterance],
    first: &[Line],
    ids: &[String],
    glossary: &[&str],
    progress: &dyn Fn(usize, usize),
) -> Adjudication {
    let system = format!("{}{SECOND_PASS}", prompt::SYSTEM);
    let mut result = Adjudication::default();
    let batches = ids.len().div_ceil(BATCH);
    let mut asked = 0;
    // The second round asks again for what the first left out.
    for round in 1..=2 {
        let answered: HashSet<String> = result.lines.iter().map(|l| l.id.clone()).collect();
        let todo: Vec<String> = ids
            .iter()
            .filter(|id| !answered.contains(*id))
            .cloned()
            .collect();
        if todo.is_empty() {
            break;
        }
        for batch in todo.chunks(BATCH) {
            let _purpose = inference::llm::purpose(format!(
                "words heard again, round {round}: {} lines from {}",
                batch.len(),
                batch[0]
            ));
            let message = user_message(glossary, sheet, first, batch);
            ask_with(model, &system, &message, &batch[0], &mut result);
            asked += 1;
            progress(asked.min(batches), batches);
        }
    }
    let asked: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut seen = HashSet::new();
    result
        .lines
        .retain(|l| asked.contains(l.id.as_str()) && seen.insert(l.id.clone()));
    result
}

/// The first pass's lines with the second pass's answers put in their place.
pub fn merge(first: &[Line], second: &[Line]) -> Vec<Line> {
    let replaced: HashMap<&str, &Line> = second.iter().map(|l| (l.id.as_str(), l)).collect();
    first
        .iter()
        .map(|l| {
            replaced
                .get(l.id.as_str())
                .map_or_else(|| l.clone(), |r| (*r).clone())
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/redecode.rs"]
mod tests;
