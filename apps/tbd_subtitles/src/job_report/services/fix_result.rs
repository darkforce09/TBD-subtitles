//! What Fix It did to a finished job, from its `fix.json` record, the corrections and the
//! problems the job has now.
//!
//! **Role:** count the lines Claude changed, answered unchanged, had turned down or left to the
//! owner's correction, and what the owner did with its changes; name the problems it cleared and
//! the ones left; pick the first changes' words as examples.
//!
//! **Position:** called by `report_loading::load` when the job has a Fix It record of the job as
//! it stands; the result is drawn by the file card's result card.
//!
//! **Signals and state:** none; pure.
//!
//! **Invariants:** a line counts as changed only when its change reached the corrections and
//! differs from before; a problem counts as cleared when no problem of its kind is left, so a
//! smaller count of the same kind is still left; no record of the problems before the first run
//! clears nothing.

use std::mem::discriminant;

use job_model::outputs::{Chosen, Corrections, FixRecord, FixVerdict, LineFix};

use crate::job_report::models::fix_result::FixResult;
use crate::job_report::models::problem::Problem;
use crate::job_report::services::line_counts;
use crate::settings::models::claude_models;

/// How many changes the result shows in words.
const EXAMPLES: usize = 2;

/// What `record`'s run did, with the owner's `corrections` and the problems the job has now,
/// `problems_now`.
pub(crate) fn fix_result(
    record: &FixRecord,
    corrections: &Corrections,
    problems_now: &[Problem],
) -> FixResult {
    let changed_in = |line: &&LineFix| line.applied && line.changed();
    let chosen = |line: &LineFix| corrections.get(&line.id).map(|c| &c.chosen);
    let count = |keep: &dyn Fn(&LineFix) -> bool| record.lines.iter().filter(|l| keep(l)).count();
    let mut changed: Vec<&LineFix> = record.lines.iter().filter(changed_in).collect();
    changed.sort_by(|a, b| a.id.cmp(&b.id));
    let examples: Vec<(String, String)> = changed
        .iter()
        .filter_map(|line| stages::fix_it::changed_words(line))
        .take(EXAMPLES)
        .collect();
    let cleared = record.before.as_ref().map_or_else(Vec::new, |before| {
        line_counts::problems_of(
            &before.counts,
            before.first_uncovered_s,
            before.cps_ok_share,
            before.cues,
        )
        .into_iter()
        .filter(|was| {
            problems_now
                .iter()
                .all(|now| discriminant(now) != discriminant(was))
        })
        .collect()
    });
    FixResult {
        model: claude_models::display_name(&record.model),
        changed: changed.len(),
        already_right: count(&|l| l.verdict.answered() && !(l.applied && l.changed())),
        turned_down: count(&|l| matches!(l.verdict, FixVerdict::TurnedDown { .. })),
        kept_yours: count(&|l| l.verdict.writes_correction() && !l.applied),
        owner_kept: count(&|l| l.applied && matches!(chosen(l), Some(Chosen::KeptFixIt { .. }))),
        owner_undid: count(&|l| {
            l.applied
                && corrections.by_owner(&l.id)
                && !matches!(chosen(l), Some(Chosen::KeptFixIt { .. }))
        }),
        cleared,
        left: problems_now.to_vec(),
        more: changed.len().saturating_sub(examples.len()),
        examples,
    }
}

#[cfg(test)]
#[path = "tests/fix_result.rs"]
mod tests;
