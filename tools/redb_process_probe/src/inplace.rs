//! The `inplace` command: an rkyv archive stored as a redb value and read back in place.
//!
//! **Role:** proves that an archived record can be accessed straight from the slice a redb
//! `AccessGuard` lends, with no copy, at whatever address redb hands out, and that it
//! deserializes to the value that was stored.
//!
//! **Position:** used by `main`; depends on `open_mode`, `redb` and `rkyv` (built with
//! `unaligned`, so archives have alignment 1).
//!
//! **Signals and state:** writes one database file in the folder and removes it before
//! returning, whether the check held or not.
//!
//! **Invariants:** the archive is validated (`rkyv::access`) before any field is read; the
//! deserialized record equals the stored one, from the redb slice and from a copy that starts at
//! an odd address, which is checked, so the check fails rather than pass on an aligned copy.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, ensure};
use redb::{ReadableDatabase, ReadableTable, TableDefinition};
use rkyv::rancor::Error as RkyvError;

use crate::open_mode::{Sharing, builder};

/// The table the archived record is stored in, as raw bytes.
const RECORD_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("records");

/// The key of the one record.
const RECORD_KEY: &str = "step";

/// A record shaped like a job's step record: a name, a fingerprint, times, notes and counts.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, PartialEq)]
pub(crate) struct ProbeRecord {
    pub(crate) name: String,
    pub(crate) fingerprint: u128,
    pub(crate) seconds: f64,
    pub(crate) peak_vram_mib: Option<f64>,
    pub(crate) notes: Vec<String>,
    pub(crate) counts: BTreeMap<String, u64>,
}

/// The record the command stores.
pub(crate) fn sample_record() -> ProbeRecord {
    ProbeRecord {
        name: "text_inpaint".to_string(),
        fingerprint: 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210,
        seconds: 294.25,
        peak_vram_mib: Some(3172.5),
        notes: vec![
            "15 of 21 candidates replaced".to_string(),
            "LaMa under the GPU lock".to_string(),
        ],
        counts: BTreeMap::from([
            ("frames".to_string(), 50_000),
            ("occurrences".to_string(), 21),
        ]),
    }
}

/// Creates `dir` when missing; an error here means the check cannot run at all.
pub(crate) fn prepare(dir: &Path) -> anyhow::Result<&Path> {
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    Ok(dir)
}

/// Runs the check in `dir`, which [`prepare`] made, and returns the output lines, the last one
/// `inplace: ok`.
pub(crate) fn run(dir: &Path) -> anyhow::Result<Vec<String>> {
    let path = dir.join("inplace.redb");
    let _ = std::fs::remove_file(&path);
    let result = check(&path);
    let _ = std::fs::remove_file(&path);
    result
}

fn check(path: &Path) -> anyhow::Result<Vec<String>> {
    let record = sample_record();
    let archive = rkyv::to_bytes::<RkyvError>(&record)?;
    let mut lines = vec![format!("archived {} bytes", archive.len())];
    {
        let db = builder(Sharing::Exclusive, None).create(path)?;
        let transaction = db.begin_write()?;
        transaction
            .open_table(RECORD_TABLE)?
            .insert(RECORD_KEY, archive.as_slice())?;
        transaction.commit()?;
    }
    let db = builder(Sharing::Exclusive, None).create(path)?;
    let transaction = db.begin_read()?;
    let table = transaction.open_table(RECORD_TABLE)?;
    let guard = ReadableTable::get(&table, RECORD_KEY)?.context("the record is missing")?;
    let slice: &[u8] = guard.value();
    lines.push(format!(
        "redb value: {} bytes at address {:#x} (mod 16 = {})",
        slice.len(),
        slice.as_ptr() as usize,
        slice.as_ptr() as usize % 16
    ));
    lines.extend(read_in_place("redb slice", slice, &record)?);

    let copy = odd_copy(slice);
    let shifted = copy.slice();
    ensure!(
        (shifted.as_ptr() as usize) % 2 == 1,
        "shifted copy: starts at the even address {:#x}",
        shifted.as_ptr() as usize
    );
    lines.push(format!(
        "shifted copy: {} bytes at address {:#x} (mod 16 = {})",
        shifted.len(),
        shifted.as_ptr() as usize,
        shifted.as_ptr() as usize % 16
    ));
    lines.extend(read_in_place("shifted copy", shifted, &record)?);
    lines.push("inplace: ok".to_string());
    Ok(lines)
}

/// A copy of some bytes placed in a buffer so that the copy starts at an odd address.
pub(crate) struct OddCopy {
    buffer: Vec<u8>,
    start: usize,
    len: usize,
}

impl OddCopy {
    /// The copied bytes, exactly as long as the original.
    pub(crate) fn slice(&self) -> &[u8] {
        &self.buffer[self.start..self.start + self.len]
    }
}

/// `bytes` copied one byte into or at the start of a buffer, whichever makes its start odd.
pub(crate) fn odd_copy(bytes: &[u8]) -> OddCopy {
    let mut buffer = vec![0u8; bytes.len() + 1];
    let start = usize::from((buffer.as_ptr() as usize).is_multiple_of(2));
    buffer[start..start + bytes.len()].copy_from_slice(bytes);
    OddCopy {
        buffer,
        start,
        len: bytes.len(),
    }
}

/// Validates the archive in `bytes` where it lies, reads each field, and checks that it
/// deserializes to `expected`.
fn read_in_place(label: &str, bytes: &[u8], expected: &ProbeRecord) -> anyhow::Result<Vec<String>> {
    let archived = rkyv::access::<ArchivedProbeRecord, RkyvError>(bytes)
        .with_context(|| format!("{label}: rkyv::access"))?;
    let notes: Vec<&str> = archived.notes.iter().map(|note| note.as_str()).collect();
    let counts: Vec<String> = archived
        .counts
        .iter()
        .map(|(key, value)| format!("{}={}", key.as_str(), value.to_native()))
        .collect();
    let lines = vec![
        format!(
            "{label}: accessed in place: name={:?} fingerprint={:#x} seconds={} peak_vram_mib={:?}",
            archived.name.as_str(),
            archived.fingerprint.to_native(),
            archived.seconds.to_native(),
            archived
                .peak_vram_mib
                .as_ref()
                .map(|value| value.to_native()),
        ),
        format!("{label}: notes={notes:?} counts=[{}]", counts.join(", ")),
    ];
    let back = rkyv::deserialize::<ProbeRecord, RkyvError>(archived)
        .with_context(|| format!("{label}: rkyv::deserialize"))?;
    ensure!(
        &back == expected,
        "{label}: deserialized {back:?}, stored {expected:?}"
    );
    Ok(lines)
}

#[cfg(test)]
#[path = "tests/inplace.rs"]
mod tests;
