//! Rows of `job.redb`: rkyv archives written in one transaction and read back typed, in place or
//! as bytes.
//!
//! **Role:** `StoreWrite` puts, reserves, reads back, removes and clears rows inside one write
//! transaction that commits them together; `StoreRead` gets a row back as its type, views its archive in place,
//! copies its bytes or lists a table's keys, all from one snapshot.
//!
//! **Position:** made by `JobStore::write` and `JobStore::read`; uses `tables` for each table's
//! definition.
//!
//! **Signals and state:** a `StoreWrite` holds the database's one write transaction until it
//! commits or drops (a drop aborts it); a `StoreRead` holds a snapshot.
//!
//! **Invariants:** a key whose kind does not match its table is an error, never a panic; every
//! value read is checked by rkyv before use; nothing a `StoreWrite` did is visible until
//! `commit` returns.

use std::io;
use std::path::{Path, PathBuf};

use redb::{ReadTransaction, ReadableTable, ReadableTableMetadata, WriteTransaction};
use rkyv::api::high::{HighDeserializer, HighSerializer, HighValidator};
use rkyv::bytecheck::CheckBytes;
use rkyv::rancor::Error as ArchiveError;
use rkyv::ser::allocator::ArenaHandle;
use rkyv::util::AlignedVec;
use rkyv::{Archive, Deserialize, Serialize};
use worker_channel::address::{Key, Table};

use super::tables::{self, Definition};
use crate::error::{Context, PipelineError, Result};

/// A table's definition with the key in the form it takes.
enum Row<'k> {
    Named(tables::NamedTable, &'k str),
    Framed(tables::FramedTable, &'k str, u64),
}

/// `key` in `table`, or an error naming the mismatch when its kind is not the table's.
fn row<'k>(table: Table, key: &'k Key, database: &Path) -> Result<Row<'k>> {
    match (tables::definition(table), key) {
        (Definition::Named(definition), Key::Name(name)) => Ok(Row::Named(definition, name)),
        (Definition::Framed(definition), Key::Frame { occurrence, frame }) => {
            Ok(Row::Framed(definition, occurrence, *frame))
        }
        (Definition::Named(_), Key::Frame { .. }) => Err(PipelineError::new(
            context(database, table),
            "a per-frame key addresses a table keyed by name",
        )),
        (Definition::Framed(_), Key::Name(_)) => Err(PipelineError::new(
            context(database, table),
            "a named key addresses a table keyed by frame",
        )),
    }
}

fn context(database: &Path, table: Table) -> String {
    format!("job database {}, table {table}", database.display())
}

/// One write transaction of a job's database.
pub struct StoreWrite {
    transaction: WriteTransaction,
    database: PathBuf,
}

impl StoreWrite {
    pub(super) fn new(transaction: WriteTransaction, database: PathBuf) -> StoreWrite {
        StoreWrite {
            transaction,
            database,
        }
    }

    /// Archive `value` into `table` under `key`, replacing any row there.
    pub fn put<T>(&mut self, table: Table, key: &Key, value: &T) -> Result<()>
    where
        T: for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, ArchiveError>>,
    {
        let bytes = rkyv::to_bytes::<ArchiveError>(value).context(format!(
            "{}: cannot archive {key:?}",
            context(&self.database, table)
        ))?;
        self.reserve(table, key, bytes.len(), |slot| {
            slot.copy_from_slice(&bytes);
            Ok(())
        })
    }

    /// Reserve `len` bytes for the row of `key` in `table` and let `fill` write them in place,
    /// such as straight from a pipe. An error from `fill` fails the write and leaves no row for
    /// `key` in this transaction; drop the transaction to keep the row it replaced.
    pub fn reserve(
        &mut self,
        table: Table,
        key: &Key,
        len: usize,
        fill: impl FnOnce(&mut [u8]) -> io::Result<()>,
    ) -> Result<()> {
        let at = context(&self.database, table);
        match row(table, key, &self.database)? {
            Row::Named(definition, name) => {
                let mut open = self.transaction.open_table(definition).context(&at)?;
                let mut slot = open.insert_reserve(name, len).context(&at)?;
                let filled = fill(slot.as_mut());
                drop(slot);
                if filled.is_err() {
                    open.remove(name).context(&at)?;
                }
                filled.context(format!("{at}: cannot fill {key:?}"))
            }
            Row::Framed(definition, occurrence, frame) => {
                let mut open = self.transaction.open_table(definition).context(&at)?;
                let mut slot = open.insert_reserve((occurrence, frame), len).context(&at)?;
                let filled = fill(slot.as_mut());
                drop(slot);
                if filled.is_err() {
                    open.remove((occurrence, frame)).context(&at)?;
                }
                filled.context(format!("{at}: cannot fill {key:?}"))
            }
        }
    }

    /// Remove the row of `key` from `table`; whether there was one.
    pub fn remove(&mut self, table: Table, key: &Key) -> Result<bool> {
        let at = context(&self.database, table);
        match row(table, key, &self.database)? {
            Row::Named(definition, name) => {
                let mut open = self.transaction.open_table(definition).context(&at)?;
                Ok(open.remove(name).context(&at)?.is_some())
            }
            Row::Framed(definition, occurrence, frame) => {
                let mut open = self.transaction.open_table(definition).context(&at)?;
                Ok(open.remove((occurrence, frame)).context(&at)?.is_some())
            }
        }
    }

    /// The row of `key` in `table` as this transaction sees it, checked and copied out of its
    /// archive; a read-change-write in one transaction sees no other writer's change.
    pub fn get<T>(&self, table: Table, key: &Key) -> Result<Option<T>>
    where
        T: Archive,
        T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>
            + Deserialize<T, HighDeserializer<ArchiveError>>,
    {
        let at = context(&self.database, table);
        let read = |bytes: &[u8]| {
            rkyv::from_bytes::<T, ArchiveError>(bytes).context(format!("{at}: cannot read {key:?}"))
        };
        match row(table, key, &self.database)? {
            Row::Named(definition, name) => {
                let open = self.transaction.open_table(definition).context(&at)?;
                let found = open.get(name).context(&at)?;
                found.map(|guard| read(guard.value())).transpose()
            }
            Row::Framed(definition, occurrence, frame) => {
                let open = self.transaction.open_table(definition).context(&at)?;
                let found = open.get((occurrence, frame)).context(&at)?;
                found.map(|guard| read(guard.value())).transpose()
            }
        }
    }

    /// Remove every row of `table`; how many there were.
    pub fn clear(&mut self, table: Table) -> Result<u64> {
        let at = context(&self.database, table);
        match tables::definition(table) {
            Definition::Named(definition) => {
                let mut open = self.transaction.open_table(definition).context(&at)?;
                let count = open.len().context(&at)?;
                open.retain(|_, _| false).context(&at)?;
                Ok(count)
            }
            Definition::Framed(definition) => {
                let mut open = self.transaction.open_table(definition).context(&at)?;
                let count = open.len().context(&at)?;
                open.retain(|_, _| false).context(&at)?;
                Ok(count)
            }
        }
    }

    /// Commit every row this transaction wrote, at once.
    pub fn commit(self) -> Result<()> {
        self.transaction.commit().context(format!(
            "job database {}: cannot commit",
            self.database.display()
        ))
    }
}

/// One read snapshot of a job's database.
pub struct StoreRead {
    transaction: ReadTransaction,
    database: PathBuf,
}

impl StoreRead {
    pub(super) fn new(transaction: ReadTransaction, database: PathBuf) -> StoreRead {
        StoreRead {
            transaction,
            database,
        }
    }

    /// The row of `key` in `table` as a `T`, checked and copied out of its archive.
    pub fn get<T>(&self, table: Table, key: &Key) -> Result<Option<T>>
    where
        T: Archive,
        T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>
            + Deserialize<T, HighDeserializer<ArchiveError>>,
    {
        let at = context(&self.database, table);
        self.with_bytes(table, key, |bytes| {
            rkyv::from_bytes::<T, ArchiveError>(bytes).context(format!("{at}: cannot read {key:?}"))
        })
    }

    /// `f` of the row of `key` in `table`, read in place from its checked archive.
    pub fn view<T, R>(
        &self,
        table: Table,
        key: &Key,
        f: impl FnOnce(&T::Archived) -> R,
    ) -> Result<Option<R>>
    where
        T: Archive,
        T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>,
    {
        let at = context(&self.database, table);
        self.with_bytes(table, key, |bytes| {
            let archived = rkyv::access::<T::Archived, ArchiveError>(bytes)
                .context(format!("{at}: cannot read {key:?}"))?;
            Ok(f(archived))
        })
    }

    /// The bytes of the row of `key` in `table`, unchecked.
    pub fn raw(&self, table: Table, key: &Key) -> Result<Option<Vec<u8>>> {
        self.with_bytes(table, key, |bytes| Ok(bytes.to_vec()))
    }

    /// Every key of `table`, in key order.
    pub fn keys(&self, table: Table) -> Result<Vec<Key>> {
        let at = context(&self.database, table);
        let mut keys = Vec::new();
        match tables::definition(table) {
            Definition::Named(definition) => {
                let open = self.transaction.open_table(definition).context(&at)?;
                for entry in open.iter().context(&at)? {
                    let (key, _) = entry.context(&at)?;
                    keys.push(Key::Name(key.value().to_string()));
                }
            }
            Definition::Framed(definition) => {
                let open = self.transaction.open_table(definition).context(&at)?;
                for entry in open.iter().context(&at)? {
                    let (key, _) = entry.context(&at)?;
                    let (occurrence, frame) = key.value();
                    keys.push(Key::Frame {
                        occurrence: occurrence.to_string(),
                        frame,
                    });
                }
            }
        }
        Ok(keys)
    }

    /// `f` of the bytes of the row of `key` in `table`, read in place and unchecked.
    pub(crate) fn with_bytes<R>(
        &self,
        table: Table,
        key: &Key,
        f: impl FnOnce(&[u8]) -> Result<R>,
    ) -> Result<Option<R>> {
        let at = context(&self.database, table);
        match row(table, key, &self.database)? {
            Row::Named(definition, name) => {
                let open = self.transaction.open_table(definition).context(&at)?;
                let found = open.get(name).context(&at)?;
                found.map(|guard| f(guard.value())).transpose()
            }
            Row::Framed(definition, occurrence, frame) => {
                let open = self.transaction.open_table(definition).context(&at)?;
                let found = open.get((occurrence, frame)).context(&at)?;
                found.map(|guard| f(guard.value())).transpose()
            }
        }
    }
}
