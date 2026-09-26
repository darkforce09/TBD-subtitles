//! Unpacking a pinned runtime archive (`.tar.xz` or `.tgz`) into a runtime folder.
//!
//! **Role:** download a runtime archive (NVIDIA's `.tar.xz` redistributables, Microsoft's ONNX
//! Runtime `.tgz`) through `download.rs`, decompress it on one thread (lzma-rs or flate2) while
//! the tar reader unpacks it on another, strip the archive's top folder, and record the archive's
//! hash in a marker so the unpack is not repeated.
//!
//! **Position:** called by `mod.rs` for each runtime archive.
//!
//! **Signals and state:** writes `runtime/<unpack_to>/` and `runtime/.fetched/<id>`; keeps the
//! downloaded archive in `runtime/.archives/` so a failed unpack does not download again.
//!
//! **Invariants:** no entry is written outside the target folder (entries with `..` or an
//! absolute path are refused); the marker is written only after the whole archive unpacked.

use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::{Component, Path, PathBuf};

use super::StoreError;
use super::download::{Progress, fetch_verified};
use super::manifest::{CUDA_BUILD_FOLDER, CUDA_FOLDER, PinnedArchive};

/// Download (when needed) and unpack `archive` under `runtime_dir`.
pub fn install(
    archive: &PinnedArchive,
    runtime_dir: &Path,
    progress: Progress<'_>,
) -> Result<(), StoreError> {
    let marker = runtime_dir.join(".fetched").join(archive.id);
    if fs::read_to_string(&marker).is_ok_and(|held| held.trim() == archive.sha256) {
        return Ok(());
    }
    let file_name = archive.url.rsplit('/').next().unwrap_or(archive.id);
    let download = runtime_dir.join(".archives").join(file_name);
    fetch_verified(
        archive.url,
        &download,
        archive.size,
        archive.sha256,
        progress,
    )?;
    let target = runtime_dir.join(archive.unpack_to);
    unpack(&download, &target)?;
    if archive.unpack_to == CUDA_FOLDER || archive.unpack_to == CUDA_BUILD_FOLDER {
        link_lib64(&target)?;
    }
    if let Some(parent) = marker.parent() {
        fs::create_dir_all(parent).map_err(|e| StoreError::io(parent, e))?;
    }
    fs::write(&marker, archive.sha256).map_err(|e| StoreError::io(&marker, e))?;
    fs::remove_file(&download).map_err(|e| StoreError::io(&download, e))
}

/// nvcc looks for its libraries in `lib64/`; the redistributable archives put them in `lib/`.
fn link_lib64(cuda_root: &Path) -> Result<(), StoreError> {
    let link = cuda_root.join("lib64");
    if fs::symlink_metadata(&link).is_ok() {
        return Ok(());
    }
    std::os::unix::fs::symlink("lib", &link).map_err(|e| StoreError::io(&link, e))
}

/// Unpack a `.tar.xz` or `.tgz` into `target`, dropping each entry's first path component.
pub fn unpack(archive: &Path, target: &Path) -> Result<(), StoreError> {
    let gzip = archive.to_string_lossy().ends_with(".tgz");
    let source = File::open(archive).map_err(|e| StoreError::io(archive, e))?;
    let (reader, mut writer) = std::io::pipe().map_err(|e| StoreError::io(archive, e))?;
    let label = archive.to_path_buf();
    let decoder = std::thread::spawn(move || -> Result<(), String> {
        let mut input = BufReader::new(source);
        let result = if gzip {
            std::io::copy(&mut flate2::read::GzDecoder::new(input), &mut writer)
                .map(|_| ())
                .map_err(|e| e.to_string())
        } else {
            lzma_rs::xz_decompress(&mut input, &mut writer).map_err(|e| e.to_string())
        };
        let _ = writer.flush();
        result
    });
    let unpacked = unpack_entries(reader, target, &label);
    let decoded = decoder
        .join()
        .unwrap_or_else(|_| Err("the decoder thread panicked".to_string()));
    unpacked?;
    decoded.map_err(|message| StoreError::Archive {
        path: label,
        message,
    })
}

fn unpack_entries(
    reader: std::io::PipeReader,
    target: &Path,
    label: &Path,
) -> Result<(), StoreError> {
    let bad = |message: String| StoreError::Archive {
        path: label.to_path_buf(),
        message,
    };
    fs::create_dir_all(target).map_err(|e| StoreError::io(target, e))?;
    let mut tar = tar::Archive::new(reader);
    let entries = tar.entries().map_err(|e| bad(e.to_string()))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| bad(e.to_string()))?;
        let raw = entry.path().map_err(|e| bad(e.to_string()))?.into_owned();
        let Some(inner) = strip_first(&raw) else {
            continue;
        };
        let dest = target.join(&inner);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| StoreError::io(parent, e))?;
        }
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&dest).map_err(|e| StoreError::io(&dest, e))?;
            continue;
        }
        // Archives unpacked into one folder share names such as LICENSE; the later one wins.
        if fs::symlink_metadata(&dest).is_ok() {
            fs::remove_file(&dest).map_err(|e| StoreError::io(&dest, e))?;
        }
        entry
            .unpack(&dest)
            .map_err(|e| bad(format!("{}: {e}", inner.display())))?;
    }
    Ok(())
}

/// The entry path without its first component, or `None` for the top folder itself and for any
/// path that would leave the target.
pub fn strip_first(path: &Path) -> Option<PathBuf> {
    let mut parts = path.components();
    match parts.next() {
        Some(Component::Normal(_)) => {}
        _ => return None,
    }
    let mut inner = PathBuf::new();
    for part in parts {
        match part {
            Component::Normal(name) => inner.push(name),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!inner.as_os_str().is_empty()).then_some(inner)
}
