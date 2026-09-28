//! The job's corrections, `review.json`, read and changed by two writers in turn: the window's
//! line review and Fix It.
//!
//! **Role:** read the corrections, change them under the job's corrections lock so no writer
//! overwrites another's change, and give the digest the review step's fingerprint covers.
//!
//! **Position:** used by the job runner, Fix It and the window's line review.
//!
//! **Signals and state:** takes an exclusive lock on `review.json.lock` for the length of one
//! change; writes or removes `review.json`.
//!
//! **Invariants:** every change reads the file as it is on disk under the lock, so a change made
//! meanwhile by the other writer is kept; nothing is written when nothing changed; the file goes
//! once no correction is left.

use std::fs::{self, OpenOptions};

use job_model::outputs::Corrections;
use sha2::{Digest, Sha256};

use super::{WorkDir, read_json, write_json};
use crate::error::{Context, Result};

/// The job's corrections; none when it has no `review.json`.
pub fn read_corrections(work: &WorkDir) -> Result<Corrections> {
    if work.review().exists() {
        read_json(&work.review())
    } else {
        Ok(Corrections::default())
    }
}

/// Read the corrections, change them with `change` and write them back, under the corrections
/// lock. The corrections as they now are, and what `change` returned.
pub fn update_corrections<R>(
    work: &WorkDir,
    change: impl FnOnce(&mut Corrections) -> R,
) -> Result<(Corrections, R)> {
    fs::create_dir_all(work.root()).context(format!("cannot create {}", work.root().display()))?;
    let path = work.corrections_lock();
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .context(format!("cannot open {}", path.display()))?;
    lock.lock()
        .context(format!("cannot lock {}", path.display()))?;
    let before = read_corrections(work)?;
    let mut corrections = before.clone();
    let result = change(&mut corrections);
    if corrections != before {
        if corrections.lines.is_empty() {
            match fs::remove_file(work.review()) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    return Err(e).context(format!("cannot remove {}", work.review().display()));
                }
                _ => {}
            }
        } else {
            write_json(&work.review(), &corrections)?;
        }
    }
    // Dropping the file releases the lock.
    drop(lock);
    Ok((corrections, result))
}

/// The SHA-256 of the corrections file, or `None` when the job has none.
pub fn corrections_digest(work: &WorkDir) -> Option<String> {
    let bytes = fs::read(work.review()).ok()?;
    Some(
        Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

#[cfg(test)]
#[path = "tests/corrections.rs"]
mod tests;
