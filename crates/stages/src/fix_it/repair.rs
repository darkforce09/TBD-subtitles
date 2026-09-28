//! The second pass: one problem family's lines, fixed in batches, each answer guarded before it
//! changes the line.
//!
//! **Role:** ask the model about one family's items (the brief first, then a block per line),
//! hold each answered line against the guard, and change the working copy, note that the line
//! may be kept as it is, or record the rule the answer broke.
//!
//! **Position:** called by `fix_it::run` once per family, words first, then timing and layout,
//! then reading speed, so each family sees the lines as the earlier ones left them.
//!
//! **Signals and state:** one call per batch of `BATCH` lines, several at once; changes the
//! drafts in place.
//!
//! **Invariants:** a family with no items makes no call; only an id the batch asked about is
//! read, once; a change reaches the draft only through the guard.

use std::collections::{HashMap, HashSet};

use job_model::outputs::{FixFamily, FixStep};
use serde::Deserialize;

use super::calls::Calls;
use super::evidence::{self, Lookup};
use super::guard::{self, Answer, Outcome};
use super::items::Item;
use super::{Draft, Episode, prompt};

/// Lines per repair call.
pub const BATCH: usize = 25;

/// A repair answer.
#[derive(Debug, Deserialize)]
struct Answers {
    lines: Vec<Answer>,
}

/// Fix `items`, all of `family`, in `drafts`; `progress` hears `(calls done, calls)`.
pub(crate) fn repair(
    ep: &Episode,
    context: &str,
    family: FixFamily,
    items: &[&Item],
    drafts: &mut HashMap<String, Draft>,
    calls: &Calls,
    progress: &(dyn Fn(usize, usize) + Sync),
) {
    if items.is_empty() || calls.stopped() {
        return;
    }
    let lookup = Lookup::new(ep);
    let batches: Vec<&[&Item]> = items.chunks(BATCH).collect();
    let messages: Vec<String> = batches
        .iter()
        .map(|batch| message(context, &lookup, batch, drafts))
        .collect();
    let label = format!("{} fixes", family.describe());
    let answers = calls.ask_all(
        &label,
        &prompt::repair(family),
        &prompt::repair_schema(),
        &messages,
        progress,
    );
    for (batch, answer) in batches.iter().zip(answers) {
        let Some(answer) = answer.and_then(|value| calls.read::<Answers>(&label, value)) else {
            continue;
        };
        let asked: HashMap<&str, &Item> = batch.iter().map(|i| (i.id.as_str(), *i)).collect();
        let mut seen = HashSet::new();
        for line in answer.lines {
            let Some(item) = asked.get(line.id.as_str()) else {
                continue;
            };
            if !seen.insert(line.id.clone()) {
                continue;
            }
            if let Some(draft) = drafts.get_mut(&line.id) {
                apply(ep, family, item, &line, draft);
            }
        }
    }
}

/// One answered line against the guard, into its draft.
fn apply(ep: &Episode, family: FixFamily, item: &Item, line: &Answer, draft: &mut Draft) {
    let why = line.why.trim().to_string();
    match guard::check(
        family,
        line,
        &draft.text,
        &draft.flags,
        ep.sheet,
        ep.glossary,
    ) {
        Ok(Outcome::Keep) => {
            if item.keep_settles && draft.kept.is_none() {
                draft.kept = Some(match family {
                    FixFamily::Timing => format!("Timed again alone. {why}"),
                    _ => why,
                });
            }
        }
        Ok(Outcome::Change { text, flags }) => {
            draft.text.clone_from(&text);
            draft.flags.clone_from(&flags);
            draft.steps.push(FixStep {
                family,
                text,
                flags,
                why,
            });
        }
        Err(rule) => draft
            .refused
            .push(format!("{} fix {rule}", family.describe())),
    }
}

/// The message for one batch: the brief, then each line's block as its draft stands now.
fn message(
    context: &str,
    lookup: &Lookup,
    batch: &[&Item],
    drafts: &HashMap<String, Draft>,
) -> String {
    let mut text = format!("{context}\nLines to fix:\n");
    for item in batch {
        let Some(draft) = drafts.get(&item.id) else {
            continue;
        };
        text.push('\n');
        text.push_str(&evidence::repair_block(
            lookup,
            draft.index,
            &item.problems,
            &draft.text,
            &draft.flags,
        ));
    }
    text
}
