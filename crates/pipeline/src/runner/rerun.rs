//! A job's start in its database: the job record of this run, and the steps asked to run again
//! cleared with every step that reads them, in one write transaction.
//!
//! **Role:** put the job record, remove the documents and records of each rerun step and of
//! every step that depends on it, and remove the per-frame rows of every cleared step that owns
//! a per-frame table; before a step runs, remove its record and its per-frame rows.
//!
//! **Position:** `start` is called by `runner::run_job` once, before the first step, and
//! `forget` before each step that runs; uses `graph` for the dependents and the tables a step
//! owns, and `work_dir::store::keys` for the rows.
//!
//! **Signals and state:** one write transaction of the job's database each; the files the
//! cleared rows named become unnamed and go on the database's next open.
//!
//! **Invariants:** either every row is cleared and the record put, or nothing changes; a
//! per-frame table (`frames` of `text_mask`, `readings` of `text_verify`) goes whole whenever the
//! step that writes it is cleared or runs again, so no row of an earlier run outlives it; a
//! cleared localized video's record keeps the path of the video it wrote beside the source, as its
//! earlier one, so a rerun may replace the file.

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::onscreen::LocalizedVideoRecord;
use worker_channel::address::Table;

use crate::error::Result;
use crate::graph;
use crate::work_dir::JobStore;
use crate::work_dir::store::keys;

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
/// records of every step [`cleared_steps`] gives for `rerun`, with the per-frame rows of each of
/// them that owns a per-frame table. The steps it cleared.
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
        for table in graph::writes_rows(*step) {
            write.clear(*table)?;
        }
    }
    if let Some(carried) = &carried {
        let key = keys::output_key(StepName::LocalizedVideo, None);
        write.put(Table::Outputs, &key, carried)?;
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

/// Remove `step`'s record and the rows of the per-frame tables it owns in a transaction of its
/// own, before the step runs again, so a run killed while the step rewrites its files never
/// resumes from the record of the files it replaced, and the step's new rows never sit beside
/// rows of frames its earlier run had and this one has not.
pub fn forget(store: &JobStore, step: StepName) -> Result<()> {
    let mut write = store.write()?;
    let mut changed = write.remove(Table::StepRecords, &keys::record_key(step))?;
    for table in graph::writes_rows(step) {
        changed |= write.clear(*table)? > 0;
    }
    if changed {
        write.commit()?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/rerun.rs"]
mod tests;
