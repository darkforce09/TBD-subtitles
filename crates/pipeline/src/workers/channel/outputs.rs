//! A step's outputs as its worker sends them: kept in one write transaction of the job database
//! and committed with the step's record.
//!
//! **Role:** `StepWrite` begins the step's write transaction on the first `Output` frame, reads
//! each archive from the worker's pipe into the row redb reserves for it, checks it against its
//! record kind, and commits every row with the step's record.
//!
//! **Position:** fed by `workers::frames::read_frames`, handed back by `workers::run_worker`, and
//! committed by the runner once the step's record is built; uses `work_dir::store`.
//!
//! **Signals and state:** holds the job's store and, from the first output on, the database's one
//! write transaction; while it is open, another step's first output waits for it.
//!
//! **Invariants:** an output whose table and key name no record kind is refused before anything is
//! read into the store; an archive that does not check leaves no row; a `StepWrite` dropped
//! without `commit` stores nothing.

use std::io::{self, Read};
use std::sync::Arc;

use job_model::StepName;
use job_model::job::StepRecord;
use worker_channel::address::{Address, Key, Table};

use crate::error::Result;
use crate::work_dir::JobStore;
use crate::work_dir::store::{self, StoreWrite, kinds};

/// The outputs a step's worker has sent so far, uncommitted.
pub struct StepWrite {
    store: Arc<JobStore>,
    write: Option<StoreWrite>,
    received: usize,
}

impl StepWrite {
    /// A step's outputs, kept in `store` once they arrive.
    pub fn new(store: Arc<JobStore>) -> StepWrite {
        StepWrite {
            store,
            write: None,
            received: 0,
        }
    }

    /// How many outputs arrived.
    pub fn received(&self) -> usize {
        self.received
    }

    /// Whether the step's write transaction has begun.
    #[cfg(test)]
    pub(crate) fn is_open(&self) -> bool {
        self.write.is_some()
    }

    /// Keep the output at `address` whose `archive_len` bytes come next on `pipe`: read them into
    /// the row the transaction reserves and check them against the row's kind. `Err` is a
    /// protocol break that names the output; the caller stops the worker.
    pub fn receive(
        &mut self,
        address: Address,
        archive_len: usize,
        pipe: &mut impl Read,
    ) -> std::result::Result<(), String> {
        let Address { table, key } = address;
        let shown = format!("{table} {}", kinds::shown(&key));
        if !writable(table) {
            return Err(format!(
                "the worker sent the output {shown}, but a worker never writes the {table} table"
            ));
        }
        let kind = store::kind(table, &key)
            .map_err(|error| format!("the worker sent the output {shown}: {error}"))?;
        let write = match &mut self.write {
            Some(write) => write,
            empty => empty.insert(
                self.store
                    .write()
                    .map_err(|error| format!("the output {shown} cannot be kept: {error}"))?,
            ),
        };
        write
            .reserve(table, &key, archive_len, |slot| {
                pipe.read_exact(slot)?;
                kind.check(slot)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
            })
            .map_err(|error| format!("the worker's output {shown} was refused: {error}"))?;
        self.received += 1;
        Ok(())
    }

    /// Put `record` under `step` in `step_records` beside the outputs and commit them together.
    pub fn commit(self, step: StepName, record: &StepRecord) -> Result<()> {
        let mut write = match self.write {
            Some(write) => write,
            None => self.store.write()?,
        };
        write.put(
            Table::StepRecords,
            &Key::Name(step.as_str().to_string()),
            record,
        )?;
        write.commit()
    }
}

impl std::fmt::Debug for StepWrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StepWrite")
            .field("database", &self.store.path())
            .field("received", &self.received)
            .finish()
    }
}

/// Whether a worker may write rows of `table`: only a step's outputs, never the job's own record,
/// the step records or the owner's corrections.
fn writable(table: Table) -> bool {
    matches!(table, Table::Outputs | Table::Frames | Table::Readings)
}
