//! The sign library shared by episodes: `library.redb` in the app's data folder, holding every
//! approved sign under its normalised Japanese and keyframe crop hash.
//!
//! **Role:** look signs up, record approved ones, remove rejected ones, measure and clear the
//! library, each in one transaction of a database no process keeps open.
//!
//! **Position:** used by `resume` (the fingerprints of `text_translate` and `text_compose`), by
//! the tasks of those two steps and of `output`, and by the window's Check Text and Settings;
//! `key` makes the keys and `signs` turns a job's documents into lookups and records.
//!
//! **Signals and state:** `library.redb`, opened read-write for one transaction and closed again;
//! while another handle holds it the open is retried every `BUSY_STEP` for up to `BUSY_WAIT`.
//! A worker finds the file through the `LOCATION_VARIABLE` its runner sets.
//!
//! **Invariants:** no handle outlives the call that opened it; a lookup, a removal and a measure
//! of a library with no file create none; a library whose stored layout differs from
//! `LAYOUT_VERSION` loses its signs whole, never migrated; every value is an rkyv archive of a
//! `LibrarySign`, checked before use.

pub mod key;
pub mod signs;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use job_model::StepName;
use job_model::onscreen::LibrarySign;
use redb::{
    Builder, Database, DatabaseError, ReadableTable, ReadableTableMetadata, TableDefinition,
    WriteTransaction,
};

use crate::error::{Context, PipelineError, Result};
use key::{MATCH_DISTANCE, distance, normalised};

/// The library's file name in the app's data folder.
pub const FILE_NAME: &str = "library.redb";
/// The variable that names the library to a worker of a step that reads it; empty for none.
pub const LOCATION_VARIABLE: &str = "TBD_SUBTITLES_LIBRARY";

type SignsTable = TableDefinition<'static, (&'static str, u64), &'static [u8]>;
/// Every sign, keyed by its normalised Japanese and crop hash.
const SIGNS: SignsTable = TableDefinition::new("signs");
/// The layout version of the signs' archives.
const META: TableDefinition<'static, &'static str, u64> = TableDefinition::new("meta");
const LAYOUT_KEY: &str = "layout";
/// Bumped whenever `LibrarySign` changes; a library at another version drops its signs.
const LAYOUT_VERSION: u64 = 1;
/// redb's page cache for the library; one transaction reads a few signs.
const CACHE_BYTES: usize = 16 * 1024 * 1024;
/// How long a call waits for another handle of the file to close.
const BUSY_WAIT: Duration = Duration::from_secs(5);
const BUSY_STEP: Duration = Duration::from_millis(50);

/// The library of every job of this user: `library.redb` in the app's data folder, beside the
/// default `work/` folder.
pub fn default_path() -> Result<PathBuf> {
    Ok(inference::model_store::app_data_dir()
        .context("cannot find the data folder")?
        .join(FILE_NAME))
}

/// Whether `step` reads the library: the translation looks signs up, and the composition starts
/// from their lettering styles.
pub fn reads_library(step: StepName) -> bool {
    matches!(step, StepName::TextTranslate | StepName::TextCompose)
}

/// How many signs the library holds and the bytes of its file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LibrarySize {
    pub signs: u64,
    pub bytes: u64,
}

/// What recording a job's approved signs did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Recorded {
    /// Signs the library did not hold.
    pub added: usize,
    /// Signs it held already, which now name the job too.
    pub joined: usize,
}

/// The library at one path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    path: PathBuf,
    wait: Duration,
}

impl Library {
    pub fn at(path: impl Into<PathBuf>) -> Library {
        Library {
            path: path.into(),
            wait: BUSY_WAIT,
        }
    }

    /// The library a worker's runner named in `LOCATION_VARIABLE`; `None` when it named none.
    pub fn from_environment() -> Option<Library> {
        std::env::var_os(LOCATION_VARIABLE)
            .filter(|path| !path.is_empty())
            .map(Library::at)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The same library, waiting at most `wait` for another handle to close.
    #[cfg(test)]
    pub(crate) fn waiting(mut self, wait: Duration) -> Library {
        self.wait = wait;
        self
    }

    /// The nearest sign of `japanese`'s normalised text whose crop hash is within
    /// `MATCH_DISTANCE` of `crop_hash`.
    pub fn lookup(&self, japanese: &str, crop_hash: u64) -> Result<Option<LibrarySign>> {
        Ok(self
            .lookup_all(&[(japanese, crop_hash)], None)?
            .pop()
            .flatten())
    }

    /// [`Library::lookup`] for each `(japanese, crop_hash)` of `wanted`, in one transaction,
    /// skipping the signs that came from the job `excluding`.
    pub fn lookup_all(
        &self,
        wanted: &[(&str, u64)],
        excluding: Option<&str>,
    ) -> Result<Vec<Option<LibrarySign>>> {
        let at = self.context("lookup");
        let found = self.transact(false, |transaction| {
            let table = transaction.open_table(SIGNS).context(&at)?;
            wanted
                .iter()
                .map(|(japanese, hash)| {
                    let keep =
                        |sign: &LibrarySign| excluding.is_none_or(|j| sign.origin() != Some(j));
                    Ok(nearest(&table, &normalised(japanese), *hash, keep, &at)?.map(|(_, s)| s))
                })
                .collect()
        })?;
        Ok(found.unwrap_or_else(|| vec![None; wanted.len()]))
    }

    /// Record one approved sign; `true` when the library did not hold it.
    pub fn record(&self, sign: LibrarySign) -> Result<bool> {
        Ok(self.record_all(vec![sign])?.added == 1)
    }

    /// Record every sign of `signs` in one transaction. A sign the library holds already (the
    /// same normalised Japanese, a crop hash within `MATCH_DISTANCE`) keeps its English, style,
    /// patch and mask, and adds the recording job to its episodes.
    pub fn record_all(&self, signs: Vec<LibrarySign>) -> Result<Recorded> {
        if signs.is_empty() {
            return Ok(Recorded::default());
        }
        let at = self.context("record");
        let recorded = self.transact(true, |transaction| {
            let mut table = transaction.open_table(SIGNS).context(&at)?;
            let mut recorded = Recorded::default();
            for sign in signs {
                let text = normalised(&sign.japanese);
                let held = nearest(&table, &text, sign.crop_hash, |_| true, &at)?;
                let (hash, value) = match held {
                    Some((hash, mut held)) => {
                        recorded.joined += 1;
                        let new = sign.episodes.iter().filter(|e| !held.episodes.contains(e));
                        let new: Vec<String> = new.cloned().collect();
                        if new.is_empty() {
                            continue;
                        }
                        held.episodes.extend(new);
                        (hash, held)
                    }
                    None => {
                        recorded.added += 1;
                        (sign.crop_hash, sign)
                    }
                };
                let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&value).context(&at)?;
                table
                    .insert((text.as_str(), hash), bytes.as_slice())
                    .context(&at)?;
            }
            Ok(recorded)
        })?;
        Ok(recorded.unwrap_or_default())
    }

    /// Remove every sign of `japanese`'s normalised text whose crop hash is within
    /// `MATCH_DISTANCE` of `crop_hash`; how many went.
    pub fn remove(&self, japanese: &str, crop_hash: u64) -> Result<usize> {
        let at = self.context("remove");
        let text = normalised(japanese);
        let removed = self.transact(false, |transaction| {
            let mut table = transaction.open_table(SIGNS).context(&at)?;
            let mut near = Vec::new();
            for row in table
                .range((text.as_str(), 0)..=(text.as_str(), u64::MAX))
                .context(&at)?
            {
                let (stored, _) = row.context(&at)?;
                let hash = stored.value().1;
                if distance(hash, crop_hash) <= MATCH_DISTANCE {
                    near.push(hash);
                }
            }
            for hash in &near {
                table.remove((text.as_str(), *hash)).context(&at)?;
            }
            Ok(near.len())
        })?;
        Ok(removed.unwrap_or(0))
    }

    /// The signs held and the file's bytes; a library with no file holds none.
    pub fn size(&self) -> Result<LibrarySize> {
        let at = self.context("size");
        let signs = self.transact(false, |transaction| {
            transaction
                .open_table(SIGNS)
                .context(&at)?
                .len()
                .context(&at)
        })?;
        let Some(signs) = signs else {
            return Ok(LibrarySize::default());
        };
        let bytes = fs::metadata(&self.path).map_or(0, |meta| meta.len());
        Ok(LibrarySize { signs, bytes })
    }

    /// Remove every sign and give the freed pages back to the file system.
    pub fn clear(&self) -> Result<()> {
        let at = self.context("clear");
        let Some(mut database) = self.open(false)? else {
            return Ok(());
        };
        let transaction = database.begin_write().context(&at)?;
        transaction.delete_table(SIGNS).context(&at)?;
        layout(&transaction, &at)?;
        transaction.commit().context(&at)?;
        database.compact().context(&at)?;
        Ok(())
    }

    /// Run `work` in one write transaction of the library and commit it; `None` without running
    /// it when the library has no file and `create` is off.
    fn transact<R>(
        &self,
        create: bool,
        work: impl FnOnce(&WriteTransaction) -> Result<R>,
    ) -> Result<Option<R>> {
        let Some(database) = self.open(create)? else {
            return Ok(None);
        };
        let at = self.context("transaction");
        let transaction = database.begin_write().context(&at)?;
        layout(&transaction, &at)?;
        let done = work(&transaction)?;
        transaction.commit().context(&at)?;
        Ok(Some(done))
    }

    /// The library's database, opened read-write, which also repairs a file a killed writer
    /// left; retried while another handle holds it. `None` when it has no file and `create` is
    /// off.
    fn open(&self, create: bool) -> Result<Option<Database>> {
        if !create && !self.path.is_file() {
            return Ok(None);
        }
        if let Some(folder) = self.path.parent() {
            fs::create_dir_all(folder).context(format!("cannot create {}", folder.display()))?;
        }
        let started = Instant::now();
        loop {
            let mut builder = Builder::new();
            builder.set_cache_size(CACHE_BYTES);
            match builder.create(&self.path) {
                Ok(database) => return Ok(Some(database)),
                Err(DatabaseError::DatabaseAlreadyOpen) if started.elapsed() < self.wait => {
                    std::thread::sleep(BUSY_STEP);
                }
                Err(DatabaseError::DatabaseAlreadyOpen) => {
                    return Err(PipelineError::new(
                        self.context("open"),
                        format!(
                            "another handle still held it after {:.1} s",
                            self.wait.as_secs_f64()
                        ),
                    ));
                }
                Err(error) => {
                    return Err(PipelineError::new(
                        self.context("open"),
                        format!("cannot open it: {error}"),
                    ));
                }
            }
        }
    }

    fn context(&self, what: &str) -> String {
        format!("sign library {}: {what}", self.path.display())
    }
}

/// Drop the signs of another layout version and store this one.
fn layout(transaction: &WriteTransaction, at: &str) -> Result<()> {
    let stored = {
        let meta = transaction.open_table(META).context(at)?;
        let found = meta.get(LAYOUT_KEY).context(at)?;
        found.map(|guard| guard.value())
    };
    if stored != Some(LAYOUT_VERSION) {
        transaction.delete_table(SIGNS).context(at)?;
        let mut meta = transaction.open_table(META).context(at)?;
        meta.insert(LAYOUT_KEY, LAYOUT_VERSION).context(at)?;
    }
    Ok(())
}

/// The nearest sign of the normalised `text` within `MATCH_DISTANCE` of `hash` that `keep`
/// accepts, with its stored hash; the lower hash wins a tie.
fn nearest(
    table: &impl ReadableTable<(&'static str, u64), &'static [u8]>,
    text: &str,
    hash: u64,
    keep: impl Fn(&LibrarySign) -> bool,
    at: &str,
) -> Result<Option<(u64, LibrarySign)>> {
    let mut best: Option<(u32, u64, LibrarySign)> = None;
    for row in table.range((text, 0)..=(text, u64::MAX)).context(at)? {
        let (stored, value) = row.context(at)?;
        let stored_hash = stored.value().1;
        let apart = distance(stored_hash, hash);
        if apart > MATCH_DISTANCE || best.as_ref().is_some_and(|(b, ..)| *b <= apart) {
            continue;
        }
        let sign = rkyv::from_bytes::<LibrarySign, rkyv::rancor::Error>(value.value()).context(
            format!("{at}: the sign {text} {stored_hash:016x} does not read"),
        )?;
        if keep(&sign) {
            best = Some((apart, stored_hash, sign));
        }
    }
    Ok(best.map(|(_, hash, sign)| (hash, sign)))
}

#[cfg(test)]
#[path = "tests/fixtures.rs"]
pub(crate) mod fixtures;

#[cfg(test)]
#[path = "tests/library.rs"]
mod tests;
