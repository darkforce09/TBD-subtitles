//! The job store: the one handle of a job's `job.redb` a process holds, and who owns it.
//!
//! **Role:** open a job's database read-write once per process, repairing a file a killed writer
//! left, name the owning process in `job.lock`, drop every table whose layout version changed,
//! and hand out write transactions and read snapshots of its rows.
//!
//! **Position:** in `work_dir`; opened by the runner and Fix It for the length of their work; uses
//! `tables` for the table definitions and `records` for the rows.
//!
//! **Signals and state:** a process-wide registry of the open stores by job folder, so every
//! caller in one process shares one handle; `job.redb` locked by redb for the life of the handle;
//! `job.lock` holding this process's pid while it owns the database.
//!
//! **Invariants:** one process at a time has a job's database open; `job.lock` is written only
//! after the open succeeds, so it never names a process that does not own the database, and it is
//! removed only while it still names this process, after the database is closed; a table whose
//! stored layout version differs from `LAYOUT_VERSIONS` is dropped whole, never migrated.

mod records;
mod tables;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use job_model::store::TableLayouts;
use redb::{Builder, Database, DatabaseError, ReadableDatabase, ReadableTable, TableHandle};
use worker_channel::address::Table;

use super::{WorkDir, write_text};
use crate::error::{Context, PipelineError, Result};

pub use records::{StoreRead, StoreWrite};
pub use tables::{FramedTable, LAYOUT_VERSIONS, NamedTable};

/// redb's page cache for every job database. An open write transaction's memory follows the page
/// cache, so this bounds the RAM of the largest step output a transaction holds
/// (`documentation/research/redb_large_transaction_memory.md`).
const CACHE_BYTES: usize = 256 * 1024 * 1024;

/// How long an open waits for this process's own earlier handle to finish closing.
const OWN_CLOSE_WAIT: Duration = Duration::from_secs(1);
const OWN_CLOSE_STEP: Duration = Duration::from_millis(20);

/// The open stores of this process, by canonical job folder.
static REGISTRY: LazyLock<Mutex<HashMap<PathBuf, Weak<JobStore>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn registry() -> MutexGuard<'static, HashMap<PathBuf, Weak<JobStore>>> {
    REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Whether an open may create the database or needs one already there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Create,
    Existing,
}

/// A job's database, open read-write in this process.
#[derive(Debug)]
pub struct JobStore {
    /// `None` only while the store drops, so the database closes before `job.lock` goes.
    database: Option<Database>,
    work: WorkDir,
    path: PathBuf,
    key: PathBuf,
}

impl JobStore {
    /// This process's handle of the database of the job in `work`, created when missing. Every
    /// caller in the process shares one handle; another process holding it is a busy error that
    /// names that process when `job.lock` does.
    pub fn open(work: &WorkDir) -> Result<Arc<JobStore>> {
        JobStore::open_as(work, Mode::Create, &LAYOUT_VERSIONS)
    }

    /// Like [`JobStore::open`], but only a database that already exists: a job with no
    /// `job.redb` is an error and nothing is created.
    pub fn open_existing(work: &WorkDir) -> Result<Arc<JobStore>> {
        JobStore::open_as(work, Mode::Existing, &LAYOUT_VERSIONS)
    }

    /// The job's work directory.
    pub fn work(&self) -> &WorkDir {
        &self.work
    }

    /// The path of `job.redb`.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A write transaction; it waits while another write transaction of this store is open.
    pub fn write(&self) -> Result<StoreWrite> {
        let transaction = self
            .database()?
            .begin_write()
            .context(self.context("cannot begin a write"))?;
        Ok(StoreWrite::new(transaction, self.path.clone()))
    }

    /// A read snapshot of every committed row.
    pub fn read(&self) -> Result<StoreRead> {
        let transaction = self
            .database()?
            .begin_read()
            .context(self.context("cannot begin a read"))?;
        Ok(StoreRead::new(transaction, self.path.clone()))
    }

    fn database(&self) -> Result<&Database> {
        self.database
            .as_ref()
            .ok_or_else(|| PipelineError::new(self.context("closed"), "the store is closing"))
    }

    fn context(&self, what: &str) -> String {
        format!("job database {}: {what}", self.path.display())
    }

    fn open_as(work: &WorkDir, mode: Mode, versions: &[(Table, u32)]) -> Result<Arc<JobStore>> {
        let root = work.root();
        if mode == Mode::Create {
            fs::create_dir_all(root).context(format!("cannot create {}", root.display()))?;
        }
        let key = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let started = Instant::now();
        loop {
            let mut open = registry();
            if let Some(live) = open.get(&key).and_then(Weak::upgrade) {
                return Ok(live);
            }
            match open_database(work, mode) {
                Ok(database) => {
                    let store = JobStore::own(database, work, key.clone(), versions)?;
                    let store = Arc::new(store);
                    open.insert(key, Arc::downgrade(&store));
                    return Ok(store);
                }
                Err(Opening::AlreadyOpen) => {
                    drop(open);
                    let owner = lock_owner(work);
                    if owner == Some(std::process::id()) && started.elapsed() < OWN_CLOSE_WAIT {
                        std::thread::sleep(OWN_CLOSE_STEP);
                        continue;
                    }
                    return Err(PipelineError::busy(
                        format!("job {}", root.display()),
                        owner,
                    ));
                }
                Err(Opening::Failed(error)) => return Err(error),
            }
        }
    }

    /// Take ownership of a freshly opened database: bring its tables to `versions`, then name
    /// this process in `job.lock`.
    fn own(
        database: Database,
        work: &WorkDir,
        key: PathBuf,
        versions: &[(Table, u32)],
    ) -> Result<JobStore> {
        let path = work.database();
        apply_layouts(&database, versions, &path)?;
        write_text(&work.lock(), &std::process::id().to_string())?;
        Ok(JobStore {
            database: Some(database),
            work: work.clone(),
            path,
            key,
        })
    }
}

impl Drop for JobStore {
    fn drop(&mut self) {
        // Holding the registry while the database closes keeps a new open in this process from
        // seeing the file still locked, and from writing its `job.lock` before this one goes.
        let mut open = registry();
        if open
            .get(&self.key)
            .is_some_and(|weak| weak.strong_count() == 0)
        {
            open.remove(&self.key);
        }
        drop(self.database.take());
        if lock_owner(&self.work) == Some(std::process::id()) {
            let _ = fs::remove_file(self.work.lock());
        }
    }
}

/// Why an open did not return a database.
enum Opening {
    /// Another handle, in this process or another, has the file open.
    AlreadyOpen,
    Failed(PipelineError),
}

fn open_database(work: &WorkDir, mode: Mode) -> std::result::Result<Database, Opening> {
    let path = work.database();
    let shown = path.display().to_string();
    let mut builder = Builder::new();
    builder.set_cache_size(CACHE_BYTES);
    builder.set_repair_callback(move |session| {
        tracing::info!(
            "repairing the job database {shown} a killed run left: {:.0}%",
            session.progress() * 100.0
        );
    });
    let opened = match mode {
        Mode::Create => builder.create(&path),
        Mode::Existing => builder.open(&path),
    };
    opened.map_err(|error| match error {
        DatabaseError::DatabaseAlreadyOpen => Opening::AlreadyOpen,
        error => Opening::Failed(PipelineError::new(
            format!("job database {}", path.display()),
            format!("cannot open it: {error}"),
        )),
    })
}

/// The pid `job.lock` names; `None` when the file is missing or holds no pid.
fn lock_owner(work: &WorkDir) -> Option<u32> {
    fs::read_to_string(work.lock()).ok()?.trim().parse().ok()
}

/// In one transaction: drop every table whose stored layout version is not its version in
/// `versions`, create every table, and store `versions` as the layout.
fn apply_layouts(database: &Database, versions: &[(Table, u32)], path: &Path) -> Result<()> {
    let at = format!("job database {}: layout", path.display());
    let transaction = database.begin_write().context(&at)?;
    let existing: HashSet<String> = transaction
        .list_tables()
        .context(&at)?
        .map(|table| table.name().to_string())
        .collect();
    let stored = if existing.contains(Table::Meta.name()) {
        let meta = transaction.open_table(tables::META).context(&at)?;
        let found = meta.get(tables::LAYOUT_KEY).context(&at)?;
        // A layout that no longer reads counts as none, so every table is dropped.
        found.and_then(|guard| {
            rkyv::from_bytes::<TableLayouts, rkyv::rancor::Error>(guard.value()).ok()
        })
    } else {
        None
    };
    let stored = stored.unwrap_or_default();
    for (table, version) in versions {
        let name = table.name();
        if existing.contains(name) && stored.versions.get(name) != Some(version) {
            tracing::info!(
                "dropping table {name} of {}: its layout changed",
                path.display()
            );
            match tables::definition(*table) {
                tables::Definition::Named(definition) => transaction.delete_table(definition),
                tables::Definition::Framed(definition) => transaction.delete_table(definition),
            }
            .context(&at)?;
        }
    }
    for table in Table::ALL {
        match tables::definition(table) {
            tables::Definition::Named(definition) => transaction.open_table(definition).map(drop),
            tables::Definition::Framed(definition) => transaction.open_table(definition).map(drop),
        }
        .context(&at)?;
    }
    let layouts = TableLayouts {
        versions: versions
            .iter()
            .map(|(table, version)| (table.name().to_string(), *version))
            .collect::<BTreeMap<_, _>>(),
    };
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&layouts).context(&at)?;
    {
        let mut meta = transaction.open_table(tables::META).context(&at)?;
        meta.insert(tables::LAYOUT_KEY, bytes.as_slice())
            .context(&at)?;
    }
    transaction.commit().context(&at)
}

#[cfg(test)]
#[path = "tests/store.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/records.rs"]
mod records_tests;
