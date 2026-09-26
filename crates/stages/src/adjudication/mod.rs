//! The language model settles each disagreement, chooses sound cues and flags doubt.
//!
//! **Role:** send the diff sheet to a language model in batches with the rules and the glossary,
//! re-ask once for any utterance a batch left out, and check the answer.
//!
//! **Position:** called inside the language-model worker (and by the stack spike tool) with any
//! `inference::llm::LanguageModel`; `prompt.rs` holds the rules and the schema, `checks.rs` the
//! checks.
//!
//! **Signals and state:** one model call per batch; the answers collected in sheet order.
//!
//! **Invariants:** the model never sees a timing; every sheet id is asked for, and the checks run
//! on the whole answer.

pub mod checks;
pub mod glossary;
pub mod prompt;
pub mod redecode;
pub mod sound_cues;
pub mod summary;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use inference::llm::{LanguageModel, LlmError};

use crate::diff_sheet::sheet::Utterance;
use checks::Line;

/// Utterances per model call.
pub const BATCH: usize = 60;

/// What a run cost and returned.
#[derive(Debug, Clone, Default)]
pub struct Adjudication {
    pub lines: Vec<Line>,
    pub calls: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    /// Batches whose answer could not be read.
    pub failed_calls: Vec<String>,
}

/// Settle `sheet` with `model`; `progress` hears `(batches done, batches)`.
pub fn adjudicate(
    model: &mut dyn LanguageModel,
    sheet: &[Utterance],
    glossary: &[&str],
    mut progress: impl FnMut(usize, usize),
) -> Adjudication {
    let mut result = Adjudication::default();
    let batches: Vec<&[Utterance]> = sheet.chunks(BATCH).collect();
    for (i, batch) in batches.iter().enumerate() {
        ask(model, batch, glossary, &mut result);
        progress(i + 1, batches.len());
    }
    finish(model, sheet, glossary, &mut result);
    result
}

/// Settle `sheet` with `workers` models at once, each made by `make`; for backends that are
/// separate processes, such as the `claude` CLI.
pub fn adjudicate_concurrently(
    make: &(dyn Fn() -> Box<dyn LanguageModel + Send> + Sync),
    workers: usize,
    sheet: &[Utterance],
    glossary: &[&str],
) -> Adjudication {
    let batches: Vec<&[Utterance]> = sheet.chunks(BATCH).collect();
    let next = AtomicUsize::new(0);
    let parts: Vec<Adjudication> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers.max(1))
            .map(|_| {
                scope.spawn(|| {
                    let mut model = make();
                    let mut part = Adjudication::default();
                    while let Some(batch) = batches.get(next.fetch_add(1, Ordering::SeqCst)) {
                        ask(model.as_mut(), batch, glossary, &mut part);
                    }
                    part
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    let mut result = Adjudication::default();
    for part in parts {
        result.lines.extend(part.lines);
        result.calls += part.calls;
        result.input_tokens += part.input_tokens;
        result.output_tokens += part.output_tokens;
        result.cost_usd += part.cost_usd;
        result.failed_calls.extend(part.failed_calls);
    }
    let mut model = make();
    finish(model.as_mut(), sheet, glossary, &mut result);
    result
}

/// Ask once more for any utterance left out, then put the lines in sheet order.
fn finish(
    model: &mut dyn LanguageModel,
    sheet: &[Utterance],
    glossary: &[&str],
    result: &mut Adjudication,
) {
    let answered: HashSet<String> = result.lines.iter().map(|l| l.id.clone()).collect();
    let missing: Vec<Utterance> = sheet
        .iter()
        .filter(|u| !answered.contains(&u.id))
        .cloned()
        .collect();
    for batch in missing.chunks(BATCH) {
        ask(model, batch, glossary, result);
    }
    let order: HashMap<&str, usize> = sheet
        .iter()
        .enumerate()
        .map(|(i, u)| (u.id.as_str(), i))
        .collect();
    result
        .lines
        .sort_by_key(|l| order.get(l.id.as_str()).copied().unwrap_or(usize::MAX));
}

fn ask(
    model: &mut dyn LanguageModel,
    batch: &[Utterance],
    glossary: &[&str],
    result: &mut Adjudication,
) {
    ask_with(
        model,
        prompt::SYSTEM,
        &prompt::user_message(glossary, batch),
        &batch[0].id,
        result,
    );
}

/// One call with the given rules and message; the answer's lines are added to `result`, or the
/// failure is recorded under `first_id`.
fn ask_with(
    model: &mut dyn LanguageModel,
    system: &str,
    user: &str,
    first_id: &str,
    result: &mut Adjudication,
) {
    result.calls += 1;
    let answer = model
        .complete_json(system, user, &prompt::schema())
        .and_then(|c| {
            let lines: Vec<Line> =
                serde_json::from_value(c.json.get("lines").cloned().unwrap_or_default())
                    .map_err(|e| LlmError(format!("answer does not match the schema: {e}")))?;
            Ok((c, lines))
        });
    match answer {
        Ok((completion, lines)) => {
            result.input_tokens += completion.input_tokens;
            result.output_tokens += completion.output_tokens;
            result.cost_usd += completion.cost_usd.unwrap_or(0.0);
            result.lines.extend(lines);
        }
        Err(e) => result.failed_calls.push(format!("{first_id}..: {e}")),
    }
}
