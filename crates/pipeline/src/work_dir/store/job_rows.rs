//! The job's own rows, typed: its record, its last run, its step records and the documents a
//! fixture puts.
//!
//! **Role:** read the job record, the last run, a step's document and every step record from a
//! snapshot; put a step's output, the job record, the last run or a step record in a transaction
//! of its own (the runner puts the last run; fixtures and tools the rest); read a job's rows from
//! its folder for a reader that holds no store (`read_stored`, `read_job`).
//!
//! **Position:** in `work_dir::store`; used by the runner, `resume`, Fix It, the app's readers and
//! the fixtures of pipeline and app tests.
//!
//! **Signals and state:** each `put_*` commits one write transaction; [`read_stored`] opens the
//! job's database for the length of the read (or shares this process's open handle), which writes
//! and removes `job.lock` as every owner does.
//!
//! **Invariants:** a row that does not read is an error, never skipped; [`read_stored`] never
//! creates a database.

use std::path::Path;

use job_model::StepName;
use job_model::job::{JobRecord, JobRun, StepRecord, StepRecords};
use rkyv::api::high::{HighDeserializer, HighSerializer, HighValidator};
use rkyv::bytecheck::CheckBytes;
use rkyv::rancor::Error as ArchiveError;
use rkyv::ser::allocator::ArenaHandle;
use rkyv::util::AlignedVec;
use worker_channel::address::{Key, Table};

use super::{JobStore, StoreRead, keys};
use crate::error::{PipelineError, Result};
use crate::work_dir::WorkDir;

/// A job as the database holds it: its record and every finished step.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredJob {
    pub record: JobRecord,
    pub steps: StepRecords,
}

impl StoreRead {
    /// The job record; `None` before the runner first wrote it.
    pub fn job_record(&self) -> Result<Option<JobRecord>> {
        self.get(Table::Meta, &keys::named(keys::JOB_RECORD))
    }

    /// The last run as a whole; `None` before a run first finished walking the steps.
    pub fn job_run(&self) -> Result<Option<JobRun>> {
        self.get(Table::Meta, &keys::named(keys::LAST_RUN))
    }

    /// `step`'s document `part`; `None` while it is not stored.
    pub fn output<T>(&self, step: StepName, part: Option<&str>) -> Result<Option<T>>
    where
        T: rkyv::Archive,
        T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>
            + rkyv::Deserialize<T, HighDeserializer<ArchiveError>>,
    {
        self.get(Table::Outputs, &keys::output_key(step, part))
    }

    /// The record of `step`; `None` while it has not finished.
    pub fn step_record(&self, step: StepName) -> Result<Option<StepRecord>> {
        self.get(Table::StepRecords, &keys::record_key(step))
    }

    /// Every step record, by step.
    pub fn step_records(&self) -> Result<StepRecords> {
        let mut steps = StepRecords::new();
        for key in self.keys(Table::StepRecords)? {
            let Key::Name(name) = &key else { continue };
            let step = name.parse::<StepName>().map_err(|error| {
                PipelineError::new(format!("step record {name}"), error.to_string())
            })?;
            if let Some(record) = self.get::<StepRecord>(Table::StepRecords, &key)? {
                steps.insert(step, record);
            }
        }
        Ok(steps)
    }
}

impl JobStore {
    /// Put `value` as `step`'s document `part` and commit it.
    pub fn put_output<T>(&self, step: StepName, part: Option<&str>, value: &T) -> Result<()>
    where
        T: for<'a> rkyv::Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, ArchiveError>>,
    {
        let mut write = self.write()?;
        write.put(Table::Outputs, &keys::output_key(step, part), value)?;
        write.commit()
    }

    /// Put the job record and commit it.
    pub fn put_job_record(&self, record: &JobRecord) -> Result<()> {
        let mut write = self.write()?;
        write.put(Table::Meta, &keys::named(keys::JOB_RECORD), record)?;
        write.commit()
    }

    /// Put the last run and commit it.
    pub fn put_job_run(&self, run: &JobRun) -> Result<()> {
        let mut write = self.write()?;
        write.put(Table::Meta, &keys::named(keys::LAST_RUN), run)?;
        write.commit()
    }

    /// Put `step`'s record and commit it.
    pub fn put_step_record(&self, step: StepName, record: &StepRecord) -> Result<()> {
        let mut write = self.write()?;
        write.put(Table::StepRecords, &keys::record_key(step), record)?;
        write.commit()
    }
}

/// The job record `store` holds; `None` before the runner first wrote it.
pub fn load_job_record(store: &JobStore) -> Result<Option<JobRecord>> {
    store.read()?.job_record()
}

/// Every step record `store` holds.
pub fn load_step_records(store: &JobStore) -> Result<StepRecords> {
    store.read()?.step_records()
}

/// `f` of a snapshot of the database of the job whose folder is `dir`, opened for the length of
/// the read (or shared with this process's open handle); `None` when the job has no database. A
/// job another process runs is a busy error.
pub fn read_stored<R>(dir: &Path, f: impl FnOnce(&StoreRead) -> Result<R>) -> Result<Option<R>> {
    let work = WorkDir::new(dir);
    if !work.database().is_file() {
        return Ok(None);
    }
    let store = JobStore::open_existing(&work)?;
    let read = store.read()?;
    f(&read).map(Some)
}

/// The job whose folder is `dir`, read from its database: `None` when it has no database or no
/// job record yet. A job another process runs is a busy error.
pub fn read_job(dir: &Path) -> Result<Option<StoredJob>> {
    let stored = read_stored(dir, |read| {
        let Some(record) = read.job_record()? else {
            return Ok(None);
        };
        Ok(Some(StoredJob {
            record,
            steps: read.step_records()?,
        }))
    })?;
    Ok(stored.flatten())
}
