//! The owner's corrections in the job database: the line corrections (`corrections/lines`),
//! changed in turn by the window's line review and Fix It, the on-screen text corrections
//! (`corrections/text`), changed by the window's text review, and Fix It's record of its runs
//! (`corrections/fix`).
//!
//! **Role:** read either set of corrections, change it in one write transaction so no writer
//! overwrites another's change, and give the digest the fingerprints of the steps that read it
//! cover.
//!
//! **Position:** used by the job runner, `resume`, Fix It and the window's line and text review,
//! each through the job's `JobStore`.
//!
//! **Signals and state:** each change is one write transaction of the job's database, which also
//! serialises it against every other writer in the process; nothing else is locked.
//!
//! **Invariants:** a change reads the row inside its own write transaction, so a change another
//! writer committed meanwhile is kept; nothing is written when nothing changed; the row goes once
//! it holds no correction, so a job without corrections has no row and no digest.

use job_model::onscreen::TextCorrections;
use job_model::outputs::{Corrections, FixRecord};
use rkyv::api::high::{HighDeserializer, HighSerializer, HighValidator};
use rkyv::bytecheck::CheckBytes;
use rkyv::rancor::Error as ArchiveError;
use rkyv::ser::allocator::ArenaHandle;
use rkyv::util::AlignedVec;
use sha2::{Digest, Sha256};
use worker_channel::address::Table;

use super::store::{JobStore, StoreRead, keys};
use crate::error::Result;

/// The job's line corrections; none when it has no row.
pub fn read_corrections(store: &JobStore) -> Result<Corrections> {
    read_row(&store.read()?, keys::LINE_CORRECTIONS)
}

/// Read the line corrections, change them with `change` and write them back, in one write
/// transaction. The corrections as they now are, and what `change` returned.
pub fn update_corrections<R>(
    store: &JobStore,
    change: impl FnOnce(&mut Corrections) -> R,
) -> Result<(Corrections, R)> {
    update_row(store, keys::LINE_CORRECTIONS, change)
}

/// The SHA-256 of the stored line corrections, or `None` when the job has none.
pub fn corrections_digest(store: &JobStore) -> Result<Option<String>> {
    digest_in(&store.read()?, keys::LINE_CORRECTIONS)
}

/// The job's on-screen text corrections; none when it has no row.
pub fn read_text_corrections(store: &JobStore) -> Result<TextCorrections> {
    read_row(&store.read()?, keys::TEXT_CORRECTIONS)
}

/// Read the on-screen text corrections, change them with `change` and write them back, in one
/// write transaction. The corrections as they now are, and what `change` returned.
pub fn update_text_corrections<R>(
    store: &JobStore,
    change: impl FnOnce(&mut TextCorrections) -> R,
) -> Result<(TextCorrections, R)> {
    update_row(store, keys::TEXT_CORRECTIONS, change)
}

/// Fix It's record of its runs; `None` before its first run.
pub fn read_fix_record(store: &JobStore) -> Result<Option<FixRecord>> {
    store.read()?.fix_record()
}

/// Keep `record` as Fix It's record of its runs, in one write transaction.
pub fn put_fix_record(store: &JobStore, record: &FixRecord) -> Result<()> {
    let mut write = store.write()?;
    write.put(Table::Corrections, &keys::named(keys::FIX_RECORD), record)?;
    write.commit()
}

impl StoreRead {
    /// The owner's line corrections in this snapshot; none when the job has none.
    pub fn line_corrections(&self) -> Result<Corrections> {
        read_row(self, keys::LINE_CORRECTIONS)
    }

    /// The owner's on-screen text corrections in this snapshot; none when the job has none.
    pub fn text_corrections(&self) -> Result<TextCorrections> {
        read_row(self, keys::TEXT_CORRECTIONS)
    }

    /// Fix It's record of its runs in this snapshot; `None` before its first run.
    pub fn fix_record(&self) -> Result<Option<FixRecord>> {
        self.get(Table::Corrections, &keys::named(keys::FIX_RECORD))
    }
}

/// The SHA-256 of the archive of the `corrections` row `name` in `read`; `None` without one.
pub(crate) fn digest_in(read: &StoreRead, name: &str) -> Result<Option<String>> {
    let found = read.raw(Table::Corrections, &keys::named(name))?;
    Ok(found.map(|bytes| {
        Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }))
}

/// The `corrections` row `name` in `read`, or the empty set without one.
pub(crate) fn read_row<T>(read: &StoreRead, name: &str) -> Result<T>
where
    T: rkyv::Archive + Default,
    T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>
        + rkyv::Deserialize<T, HighDeserializer<ArchiveError>>,
{
    Ok(read
        .get(Table::Corrections, &keys::named(name))?
        .unwrap_or_default())
}

fn update_row<T, R>(
    store: &JobStore,
    name: &str,
    change: impl FnOnce(&mut T) -> R,
) -> Result<(T, R)>
where
    T: rkyv::Archive + Default + Clone + PartialEq,
    T: for<'a> rkyv::Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, ArchiveError>>,
    T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>
        + rkyv::Deserialize<T, HighDeserializer<ArchiveError>>,
{
    let key = keys::named(name);
    let mut write = store.write()?;
    let before: T = write.get(Table::Corrections, &key)?.unwrap_or_default();
    let mut corrections = before.clone();
    let result = change(&mut corrections);
    if corrections != before {
        if corrections == T::default() {
            write.remove(Table::Corrections, &key)?;
        } else {
            write.put(Table::Corrections, &key, &corrections)?;
        }
        write.commit()?;
    }
    Ok((corrections, result))
}

#[cfg(test)]
#[path = "tests/corrections.rs"]
mod tests;
