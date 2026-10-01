//! Unpacking only some shared libraries of a large pinned `.tar.gz` while it streams past.
//!
//! **Role:** read a pinned archive once, from a copy placed in `runtime/.archives/` or straight
//! from its URL, hashing every byte as it is read; gunzip and walk the tar on the fly, and write
//! only the members [`PinnedLibraries::selects`] (files directly in a `lib/` folder) flat into a
//! staging folder; when the stream ends, check its size and SHA-256 and only then move the
//! staging folder to `runtime/<unpack_to>/` and write the marker.
//!
//! **Position:** called by `mod.rs` (`install_libraries`) for TensorRT, which only the AppImage
//! builder fetches; uses `download.rs` for the https stream and the manifest's selection.
//!
//! **Signals and state:** writes `runtime/.staging/<id>/`, then `runtime/<unpack_to>/lib/` and
//! `runtime/.fetched/<id>`; a placed archive is deleted once its libraries are installed. The
//! archive itself is never written to disk by this module.
//!
//! **Invariants:** nothing is installed from a stream whose size or hash differs from the pin:
//! the staging folder is removed on any failure; members land only under the staging `lib/`
//! folder, by file name, and symlinks point at bare file names beside them; every prefix of the
//! selection matches at least one member, or the install fails naming it.

use std::cell::Cell;
use std::fs::{self, File};
use std::io::{self, Read};
use std::ops::ControlFlow;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use super::StoreError;
use super::download::{Progress, hex, open_stream};
use super::manifest::PinnedLibraries;

/// Whether `libraries` are unpacked under `runtime_dir` with the current selection.
pub fn is_installed(libraries: &PinnedLibraries, runtime_dir: &Path) -> bool {
    fs::read_to_string(marker_path(libraries, runtime_dir))
        .is_ok_and(|held| held.trim() == libraries.marker())
}

/// Install `libraries` under `runtime_dir` unless they already are; returns the file names
/// unpacked (empty when nothing was done). The archive is read from
/// `runtime_dir/.archives/<its file name>` when a copy was placed there, else from its URL.
pub fn install(
    libraries: &PinnedLibraries,
    runtime_dir: &Path,
    progress: Progress<'_>,
) -> Result<Vec<String>, StoreError> {
    if is_installed(libraries, runtime_dir) {
        return Ok(Vec::new());
    }
    let archive = &libraries.archive;
    let placed = placed_path(libraries, runtime_dir);
    let staging = runtime_dir.join(".staging").join(archive.id);
    let names = if placed.is_file() {
        let source = File::open(&placed).map_err(|e| StoreError::io(&placed, e))?;
        unpack_verified(source, libraries, &staging, &placed, progress)?
    } else {
        let source = open_stream(archive.url)?;
        unpack_verified(
            source,
            libraries,
            &staging,
            Path::new(archive.url),
            progress,
        )?
    };
    let target = runtime_dir.join(archive.unpack_to);
    if fs::symlink_metadata(&target).is_ok() {
        fs::remove_dir_all(&target).map_err(|e| StoreError::io(&target, e))?;
    }
    fs::rename(&staging, &target).map_err(|e| StoreError::io(&target, e))?;
    let marker = marker_path(libraries, runtime_dir);
    if let Some(parent) = marker.parent() {
        fs::create_dir_all(parent).map_err(|e| StoreError::io(parent, e))?;
    }
    fs::write(&marker, libraries.marker()).map_err(|e| StoreError::io(&marker, e))?;
    if placed.is_file() {
        fs::remove_file(&placed).map_err(|e| StoreError::io(&placed, e))?;
    }
    Ok(names)
}

/// Where a copy of the archive downloaded by hand is read from instead of the URL.
pub fn placed_path(libraries: &PinnedLibraries, runtime_dir: &Path) -> PathBuf {
    let archive = &libraries.archive;
    let file_name = archive.url.rsplit('/').next().unwrap_or(archive.id);
    runtime_dir.join(".archives").join(file_name)
}

fn marker_path(libraries: &PinnedLibraries, runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(".fetched").join(libraries.archive.id)
}

/// Unpack the selected members of the `.tar.gz` read from `source` into `staging/lib/`, reading
/// `source` to its end and checking its size and hash; `staging` is emptied first and removed on
/// any failure. `label` names the source in errors.
pub(crate) fn unpack_verified(
    source: impl Read,
    libraries: &PinnedLibraries,
    staging: &Path,
    label: &Path,
    progress: Progress<'_>,
) -> Result<Vec<String>, StoreError> {
    if fs::symlink_metadata(staging).is_ok() {
        fs::remove_dir_all(staging).map_err(|e| StoreError::io(staging, e))?;
    }
    let result = unpack_and_check(source, libraries, staging, label, progress);
    if result.is_err() {
        let _ = fs::remove_dir_all(staging);
    }
    result
}

fn unpack_and_check(
    source: impl Read,
    libraries: &PinnedLibraries,
    staging: &Path,
    label: &Path,
    progress: Progress<'_>,
) -> Result<Vec<String>, StoreError> {
    let bad = |message: String| StoreError::Archive {
        path: label.to_path_buf(),
        message,
    };
    let pin = &libraries.archive;
    let cancelled = Cell::new(false);
    let mut hashing = HashingReader {
        inner: source,
        hasher: Sha256::new(),
        held: 0,
        size: pin.size,
        progress,
        cancelled: &cancelled,
    };
    let unpacked = {
        let mut tar = tar::Archive::new(flate2::read::MultiGzDecoder::new(&mut hashing));
        unpack_selected(&mut tar, libraries, &staging.join("lib"), label).and_then(|names| {
            // The tar ends before the gzip trailer; reading on checks the trailer's CRC.
            io::copy(&mut tar.into_inner(), &mut io::sink())
                .map(|_| names)
                .map_err(|e| bad(e.to_string()))
        })
    };
    if cancelled.get() {
        return Err(StoreError::Cancelled);
    }
    let names = unpacked?;
    io::copy(&mut hashing, &mut io::sink()).map_err(|e| bad(e.to_string()))?;
    if cancelled.get() {
        return Err(StoreError::Cancelled);
    }
    if hashing.held != pin.size {
        return Err(StoreError::SizeMismatch {
            path: label.to_path_buf(),
            expected: pin.size,
            found: hashing.held,
        });
    }
    let found = hex(&hashing.hasher.finalize());
    if found != pin.sha256 {
        return Err(StoreError::ChecksumMismatch {
            path: label.to_path_buf(),
            expected: pin.sha256.to_string(),
            found,
        });
    }
    if let Some(prefix) = libraries
        .prefixes
        .iter()
        .find(|p| !names.iter().any(|name| name.starts_with(*p)))
    {
        return Err(bad(format!(
            "no library in the archive starts with {prefix}"
        )));
    }
    Ok(names)
}

/// Write each selected member into `lib_dir`, by file name; returns the names written.
fn unpack_selected<R: Read>(
    tar: &mut tar::Archive<R>,
    libraries: &PinnedLibraries,
    lib_dir: &Path,
    label: &Path,
) -> Result<Vec<String>, StoreError> {
    let bad = |message: String| StoreError::Archive {
        path: label.to_path_buf(),
        message,
    };
    fs::create_dir_all(lib_dir).map_err(|e| StoreError::io(lib_dir, e))?;
    let mut names = Vec::new();
    for entry in tar.entries().map_err(|e| bad(e.to_string()))? {
        let mut entry = entry.map_err(|e| bad(e.to_string()))?;
        let path = entry.path().map_err(|e| bad(e.to_string()))?.into_owned();
        let Some(name) = library_name(&path) else {
            continue;
        };
        if !libraries.selects(&name) {
            continue;
        }
        let dest = lib_dir.join(&name);
        // A later member of the same name, from another `lib/` folder, replaces the earlier one.
        if fs::symlink_metadata(&dest).is_ok() {
            fs::remove_file(&dest).map_err(|e| StoreError::io(&dest, e))?;
        }
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            let target = link_file_name(&entry).map_err(|m| bad(format!("{name}: {m}")))?;
            if kind.is_symlink() {
                std::os::unix::fs::symlink(&target, &dest).map_err(|e| StoreError::io(&dest, e))?;
            } else {
                fs::hard_link(lib_dir.join(&target), &dest)
                    .map_err(|e| bad(format!("{name}: hard link to {target}: {e}")))?;
            }
        } else if kind.is_file() {
            entry
                .unpack(&dest)
                .map_err(|e| bad(format!("{name}: {e}")))?;
        } else {
            continue;
        }
        if !names.contains(&name) {
            names.push(name);
        }
    }
    Ok(names)
}

/// The file name a link member points at; the links are flattened like the members.
fn link_file_name<R: Read>(entry: &tar::Entry<'_, R>) -> Result<String, String> {
    let target = entry
        .link_name()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "a link without a target".to_string())?;
    target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("a link to {}", target.display()))
}

/// The file name of a member that sits directly in a folder named `lib`; `None` for every other
/// member and for any path with `..` or a root.
pub fn library_name(path: &Path) -> Option<String> {
    let mut normal = Vec::new();
    for part in path.components() {
        match part {
            Component::Normal(name) => normal.push(name),
            Component::CurDir => {}
            _ => return None,
        }
    }
    let [.., parent, name] = normal.as_slice() else {
        return None;
    };
    (*parent == "lib").then(|| name.to_string_lossy().into_owned())
}

/// Hashes and counts every byte read through it, and reports progress; a break from the
/// progress callback fails the read and sets `cancelled`.
struct HashingReader<'a, R> {
    inner: R,
    hasher: Sha256,
    held: u64,
    size: u64,
    progress: Progress<'a>,
    cancelled: &'a Cell<bool>,
}

impl<R: Read> Read for HashingReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hasher.update(&buf[..n]);
        self.held += n as u64;
        if n > 0 && (self.progress)(self.held, self.size) == ControlFlow::Break(()) {
            self.cancelled.set(true);
            return Err(io::Error::other("the download was stopped"));
        }
        Ok(n)
    }
}

#[cfg(test)]
#[path = "tests/archive_libraries.rs"]
mod tests;
