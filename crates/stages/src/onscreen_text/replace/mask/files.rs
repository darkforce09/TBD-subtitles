//! Mask and source-plate files of the stroke-mask step.
//!
//! **Role:** name each occurrence's folder, write PNG files atomically and clear stale output.
//! **Position:** used by mask extraction and its background runs.
//! **Signals and state:** one folder per occurrence under `visual/masks/`; folder names claimed
//! during one extraction.
//! **Invariants:** folder names use only `[A-Za-z0-9_-]` and are unique within a job; a file
//! appears under its final name only once it is complete and synced.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use image::{EncodableLayout, ImageBuffer, ImageFormat, PixelWithColorType};

use crate::onscreen_text::{TextResult, png};

/// The folder of every occurrence's masks and source plates, relative to the job directory.
pub(super) const MASKS: &str = "visual/masks";

/// One occurrence's folder, both on disk and as documents name it.
pub(super) struct Folder {
    absolute: PathBuf,
    relative: PathBuf,
}

impl Folder {
    /// Create the folder `name` under the job's mask folder.
    pub(super) fn create(root: &Path, name: &str) -> TextResult<Folder> {
        let relative = Path::new(MASKS).join(name);
        let absolute = root.join(&relative);
        fs::create_dir_all(&absolute).map_err(|e| format!("create {}: {e}", absolute.display()))?;
        Ok(Folder { absolute, relative })
    }

    /// Write `image` as `name` and return its job-relative path.
    pub(super) fn write<P>(
        &self,
        name: &str,
        image: &ImageBuffer<P, Vec<P::Subpixel>>,
    ) -> TextResult<PathBuf>
    where
        P: PixelWithColorType,
        [P::Subpixel]: EncodableLayout,
    {
        png::write(&self.absolute.join(name), |out| {
            image.write_to(out, ImageFormat::Png)
        })?;
        Ok(self.relative.join(name))
    }

    /// Delete the folder and everything in it.
    pub(super) fn remove(self) -> TextResult<()> {
        remove_tree(&self.absolute)
    }
}

/// Delete `dir` and everything in it; a missing folder is already clear.
pub(super) fn remove_tree(dir: &Path) -> TextResult<()> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("clear {}: {e}", dir.display()).into()),
    }
}

/// Folder names handed out during one extraction.
#[derive(Default)]
pub(super) struct Names {
    taken: HashSet<String>,
}

impl Names {
    /// A folder name for `id` that no earlier occurrence of this extraction received.
    pub(super) fn claim(&mut self, id: &str) -> String {
        let base = sanitize(id);
        let mut name = base.clone();
        let mut suffix = 2;
        while !self.taken.insert(name.clone()) {
            name = format!("{base}-{suffix}");
            suffix += 1;
        }
        name
    }
}

/// `id` with every character outside `[A-Za-z0-9_-]` replaced by `_`; never empty.
pub(super) fn sanitize(id: &str) -> String {
    let name: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        "occurrence".to_string()
    } else {
        name
    }
}

#[cfg(test)]
#[path = "tests/files.rs"]
mod tests;
