//! The squashfs image of the AppDir, written behind the AppImage runtime.
//!
//! **Role:** walk the AppDir and write it as a zstd-compressed squashfs image with the
//! `backhand` crate, at an offset in the output file so the runtime's bytes come first, keeping
//! symlinks and file modes and giving every entry owner root.
//!
//! **Position:** called by `main` after the AppDir is complete and the runtime is written.
//!
//! **Signals and state:** reads the AppDir; writes the output file from `offset` on.
//!
//! **Invariants:** entries are added in sorted path order, so the same AppDir gives the same
//! image; modification times are zero; a special file (device, fifo, socket) is an error.

use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use backhand::compression::{CompressionOptions, Compressor, Zstd};
use backhand::{FilesystemCompressor, FilesystemWriter, NodeHeader};

/// The zstd level: close to the smallest image while packing gigabytes in minutes.
const ZSTD_LEVEL: u32 = 15;

/// The data block size: 1 MiB compresses the large CUDA libraries markedly better than 128 KiB.
const BLOCK_SIZE: u32 = 1 << 20;

/// Write the image of `app_dir` into `out` starting at byte `offset`; returns the image's size.
pub(crate) fn write_image(app_dir: &Path, out: &mut File, offset: u64) -> Result<u64> {
    let mut writer = FilesystemWriter::default();
    let compressor = FilesystemCompressor::new(
        Compressor::Zstd,
        Some(CompressionOptions::Zstd(Zstd {
            compression_level: ZSTD_LEVEL,
        })),
    )
    .map_err(|e| anyhow!("choosing zstd: {e}"))?;
    writer.set_compressor(compressor);
    writer.set_block_size(BLOCK_SIZE);
    writer.set_root_mode(0o755);
    writer.set_root_uid(0);
    writer.set_root_gid(0);
    for entry in walk(app_dir)? {
        add(&mut writer, app_dir, &entry)?;
    }
    let (_, size) = writer
        .write_with_offset(out, offset)
        .map_err(|e| anyhow!("writing the squashfs image: {e}"))?;
    Ok(size)
}

/// Every path under `root`, sorted, directories before what they hold.
fn walk(root: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).with_context(|| format!("listing {}", dir.display()))? {
            let path = entry
                .with_context(|| format!("listing {}", dir.display()))?
                .path();
            let meta = fs::symlink_metadata(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            if meta.is_dir() {
                pending.push(path.clone());
            }
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

/// Add one AppDir entry to the image under its path relative to the AppDir.
fn add(writer: &mut FilesystemWriter<'_, '_, '_>, root: &Path, path: &Path) -> Result<()> {
    let inside = path
        .strip_prefix(root)
        .context("an entry outside the AppDir")?;
    let meta = fs::symlink_metadata(path).with_context(|| format!("reading {}", path.display()))?;
    let header = NodeHeader::new((meta.permissions().mode() & 0o7777) as u16, 0, 0, 0);
    let kind = meta.file_type();
    let added = if kind.is_symlink() {
        let points_at =
            fs::read_link(path).with_context(|| format!("reading the link {}", path.display()))?;
        writer.push_symlink(points_at, inside, header)
    } else if kind.is_dir() {
        writer.push_dir(inside, header)
    } else if kind.is_file() {
        writer.push_file_from_path(path, inside, header)
    } else {
        bail!("{} is not a file, folder or symlink", path.display());
    };
    added.map_err(|e| anyhow!("adding {}: {e}", inside.display()))
}

#[cfg(test)]
#[path = "tests/squashfs.rs"]
mod tests;
