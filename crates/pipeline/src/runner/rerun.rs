//! A job's start in its database: the job record of this run, and the steps asked to run again
//! cleared with every step that reads them, in one write transaction.
//!
//! **Role:** put the job record, remove the documents and records of each rerun step and of
//! every step that depends on it, and remove the per-frame rows when a cleared step owns them.
//!
//! **Position:** called by `runner::run_job` once, before the first step; uses `graph` for the
//! dependents and `work_dir::store::keys` for the rows.
//!
//! **Signals and state:** one write transaction of the job's database; the files the cleared
//! rows named become unnamed and go on the database's next open.
//!
//! **Invariants:** either every row is cleared and the record put, or nothing changes; the
//! localized video's record keeps the path of the video it wrote beside the source, as its earlier
//! one, so a rerun may replace the file; the `frames` and `readings` rows go whole whenever the
//! replacement steps that write them (`text_mask`, `text_verify`) are cleared, which every step
//! upstream of them clears too.

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::onscreen::LocalizedVideoRecord;
use worker_channel::address::Table;

use crate::error::Result;
use crate::graph;
use crate::work_dir::JobStore;
use crate::work_dir::store::keys;

/// The steps the per-frame tables belong to: clearing one of them clears both tables.
const FRAME_OWNERS: [StepName; 2] = [StepName::TextMask, StepName::TextVerify];

/// Every step `rerun` names and every step that reads one of them, in `StepName::ALL` order.
pub fn cleared_steps(rerun: &[StepName]) -> Vec<StepName> {
    StepName::ALL
        .into_iter()
        .filter(|step| {
            rerun.contains(step)
                || rerun
                    .iter()
                    .any(|asked| graph::dependents(*asked).contains(step))
        })
        .collect()
}

/// In one write transaction: put `record` as the job record, and remove the documents and
/// records of every step [`cleared_steps`] gives for `rerun`, with the per-frame rows when a
/// step that writes them is among them. The steps it cleared.
pub fn start(store: &JobStore, record: &JobRecord, rerun: &[StepName]) -> Result<Vec<StepName>> {
    let cleared = cleared_steps(rerun);
    let carried = carried_localized_video(store, &cleared)?;
    let mut write = store.write()?;
    write.put(Table::Meta, &keys::named(keys::JOB_RECORD), record)?;
    for step in &cleared {
        for key in keys::output_keys(*step) {
            write.remove(Table::Outputs, &key)?;
        }
        write.remove(Table::StepRecords, &keys::record_key(*step))?;
    }
    if let Some(carried) = &carried {
        let key = keys::output_key(StepName::LocalizedVideo, None);
        write.put(Table::Outputs, &key, carried)?;
    }
    if cleared.iter().any(|step| FRAME_OWNERS.contains(step)) {
        write.clear(Table::Frames)?;
        write.clear(Table::Readings)?;
    }
    write.commit()?;
    Ok(cleared)
}

/// The record a cleared `localized_video` leaves behind: only the video it wrote beside the source,
/// as its earlier one, so the step may replace that file when it runs again.
fn carried_localized_video(
    store: &JobStore,
    cleared: &[StepName],
) -> Result<Option<LocalizedVideoRecord>> {
    if !cleared.contains(&StepName::LocalizedVideo) {
        return Ok(None);
    }
    let key = keys::output_key(StepName::LocalizedVideo, None);
    let previous: Option<LocalizedVideoRecord> = store.read()?.get(Table::Outputs, &key)?;
    Ok(previous
        .and_then(|record| record.path.or(record.earlier))
        .map(|earlier| LocalizedVideoRecord {
            earlier: Some(earlier),
            ..LocalizedVideoRecord::default()
        }))
}

/// Remove `step`'s record in a transaction of its own, before the step runs again, so a run
/// killed while the step rewrites its files never resumes from the record of the files it
/// replaced.
pub fn forget(store: &JobStore, step: StepName) -> Result<()> {
    let mut write = store.write()?;
    if write.remove(Table::StepRecords, &keys::record_key(step))? {
        write.commit()?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/rerun.rs"]
mod tests;
