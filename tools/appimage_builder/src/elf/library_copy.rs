//! Copying a library by one of its names, with the chain of soname symlinks that leads to it.
//!
//! **Role:** copy `libfoo.so.1 -> libfoo.so.1.2.3` into another folder as the same symlinks and
//! one real file, so every name a loader or `dlopen` asks for still resolves.
//!
//! **Position:** part of `elf`; used wherever the builder bundles a library.
//!
//! **Signals and state:** reads the source folder, writes the target folder; holds nothing.
//!
//! **Invariants:** only symlinks to a file in the same folder are followed; the real file keeps
//! its mode; copying a name already present in the target is a no-op.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// How many symlinks deep a library name may lead before the chain is taken as a loop.
const MAX_LINKS: usize = 8;

/// Copy `name` from `from` into `to`, recreating its symlink chain; returns the real file's path
/// in `to`.
pub(crate) fn copy_with_links(from: &Path, name: &str, to: &Path) -> Result<PathBuf> {
    fs::create_dir_all(to).with_context(|| format!("creating {}", to.display()))?;
    let mut current = name.to_string();
    for _ in 0..MAX_LINKS {
        let source = from.join(&current);
        let target = to.join(&current);
        let meta = fs::symlink_metadata(&source)
            .with_context(|| format!("reading {}", source.display()))?;
        if !meta.file_type().is_symlink() {
            if fs::symlink_metadata(&target).is_err() {
                fs::copy(&source, &target).with_context(|| {
                    format!("copying {} to {}", source.display(), target.display())
                })?;
            }
            return Ok(target);
        }
        let next = link_file_name(&source)?;
        if fs::symlink_metadata(&target).is_err() {
            std::os::unix::fs::symlink(&next, &target)
                .with_context(|| format!("linking {}", target.display()))?;
        }
        current = next;
    }
    bail!(
        "{} leads through more than {MAX_LINKS} symlinks",
        from.join(name).display()
    )
}

/// The file name a symlink points at, which must lie in the symlink's own folder.
fn link_file_name(link: &Path) -> Result<String> {
    let points_at =
        fs::read_link(link).with_context(|| format!("reading the link {}", link.display()))?;
    let stripped = points_at.strip_prefix("./").unwrap_or(&points_at);
    if stripped.components().count() != 1 {
        bail!(
            "{} points outside its folder, at {}",
            link.display(),
            points_at.display()
        );
    }
    Ok(stripped.to_string_lossy().into_owned())
}
