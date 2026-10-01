//! Rewriting an ELF file's RUNPATH in place, so a copied library looks beside itself.
//!
//! **Role:** overwrite the RUNPATH (or RPATH) string in `.dynstr` with a shorter one such as
//! `$ORIGIN` or `$ORIGIN/../lib`, padding the rest of the old string with NUL bytes.
//!
//! **Position:** part of `elf`; `main` calls it on both workers and the libraries of theirs it
//! copies into the AppDir.
//!
//! **Signals and state:** writes the one file it is given; holds nothing.
//!
//! **Invariants:** the file's size and layout never change; a new string longer than the old one
//! is refused, as is a file with no RUNPATH or RPATH entry.

use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};

use super::read_dynamic;

/// Replace the RUNPATH (or RPATH) of the ELF file at `path` with `runpath`.
pub(crate) fn rewrite_runpath(path: &Path, runpath: &str) -> Result<()> {
    let dynamic = read_dynamic(path)?;
    let Some((old, offset)) = dynamic.runpath else {
        bail!("{} has no RUNPATH or RPATH to rewrite", path.display());
    };
    if runpath.len() > old.len() {
        bail!(
            "{}: the new RUNPATH `{runpath}` is longer than the old `{old}`",
            path.display()
        );
    }
    let mut bytes = runpath.as_bytes().to_vec();
    bytes.resize(old.len(), 0);
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .with_context(|| format!("opening {} for writing", path.display()))?;
    file.seek(SeekFrom::Start(offset))
        .and_then(|_| file.write_all(&bytes))
        .with_context(|| format!("writing the RUNPATH of {}", path.display()))
}
