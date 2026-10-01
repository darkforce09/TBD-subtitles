//! Synced PNG files of the visual steps.
//!
//! **Role:** write one PNG under a temporary name beside its final one, sync it, move it into
//! place and sync its folder.
//! **Position:** used by detection (crops and keyframe stills), stroke masks, inpainting and
//! composition; the pipeline commits the record that names the file only after the stage returns.
//! **Signals and state:** the file and its folder on disk.
//! **Invariants:** a PNG appears under its final name only whole, and it and its name are on disk
//! before the stage returns, so no record ever names a file a crash could lose; a failed write
//! leaves no temporary file behind.

use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use super::TextResult;

/// The writer a PNG encoder fills.
pub(crate) type PngWriter = BufWriter<File>;

/// Write the PNG `encode` produces as `path`, synced; `encode` is typically
/// `|out| image.write_to(out, ImageFormat::Png)`.
pub(crate) fn write(
    path: &Path,
    encode: impl FnOnce(&mut PngWriter) -> image::ImageResult<()>,
) -> TextResult<()> {
    let temporary = temporary(path);
    let written = write_synced(&temporary, encode);
    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return Err(format!("write {}: {error}", path.display()).into());
    }
    fs::rename(&temporary, path).map_err(|e| {
        let _ = fs::remove_file(&temporary);
        format!("move {} into place: {e}", path.display())
    })?;
    if let Some(folder) = path
        .parent()
        .filter(|folder| !folder.as_os_str().is_empty())
    {
        File::open(folder)
            .and_then(|folder| folder.sync_all())
            .map_err(|e| format!("sync {}: {e}", folder.display()))?;
    }
    Ok(())
}

/// `<path>.partial`.
fn temporary(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".partial");
    PathBuf::from(name)
}

fn write_synced(
    temporary: &Path,
    encode: impl FnOnce(&mut PngWriter) -> image::ImageResult<()>,
) -> Result<(), String> {
    let file = File::create(temporary).map_err(|e| e.to_string())?;
    let mut out = BufWriter::new(file);
    encode(&mut out).map_err(|e| e.to_string())?;
    let file = out.into_inner().map_err(|e| e.error().to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "tests/png.rs"]
mod tests;
