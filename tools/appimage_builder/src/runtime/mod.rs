//! The AppImage type 2 runtime, and the finished file's names in `dist/`.
//!
//! **Role:** download the pinned static `runtime-x86_64` of AppImage's type2-runtime release
//! (hash-checked, cached) that heads the AppImage and mounts the squashfs image behind it, and
//! name and finish the output: `TBD-subtitles-<version>-<commit>-x86_64.AppImage`, mode 755, plus
//! a copy under the stable name `TBD-subtitles-x86_64.AppImage`.
//!
//! **Position:** called by `main` before and after `squashfs` writes the image; uses
//! `inference::model_store` for the download and runs `git` for the commit.
//!
//! **Signals and state:** fills `target/appimage/cache/`; writes the output folder.
//!
//! **Invariants:** the runtime's SHA-256 matches the pin before it is used; the finished file is
//! executable.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use child_process::Run;
use inference::model_store;

use crate::gpu_runtime::print_progress;

/// The type2-runtime release `20251108`, static, x86_64; the hash is GitHub's asset digest.
const RUNTIME_URL: &str =
    "https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64";
const RUNTIME_SIZE: u64 = 944_632;
const RUNTIME_SHA256: &str = "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d";

/// The name every output file starts with.
const NAME: &str = "TBD-subtitles";

/// The runtime's bytes, downloading them into `cache` when needed.
pub(crate) fn fetch(cache: &Path) -> Result<Vec<u8>> {
    let path = cache.join("type2-runtime-20251108-x86_64");
    let mut progress = print_progress("runtime-x86_64");
    model_store::fetch_verified(
        RUNTIME_URL,
        &path,
        RUNTIME_SIZE,
        RUNTIME_SHA256,
        &mut progress,
    )
    .map_err(|e| anyhow!("fetching the AppImage runtime: {e}"))?;
    eprintln!();
    fs::read(&path).with_context(|| format!("reading {}", path.display()))
}

/// The versioned output name: `TBD-subtitles-0.1.0-9b528e6-x86_64.AppImage`.
pub(crate) fn versioned_name(version: &str, commit: &str) -> String {
    format!("{NAME}-{version}-{commit}-x86_64.AppImage")
}

/// The stable output name the latest build is also copied to.
pub(crate) fn stable_name() -> String {
    format!("{NAME}-x86_64.AppImage")
}

/// The app's version from the `tbd-subtitles 0.1.0` line its `--version` prints.
pub(crate) fn parse_app_version(line: &str) -> Option<&str> {
    let mut words = line.split_whitespace();
    (words.next() == Some("tbd-subtitles"))
        .then(|| words.next())
        .flatten()
}

/// The short hash of `HEAD` in `repo_root`.
pub(crate) fn short_commit(repo_root: &Path) -> Result<String> {
    let output = Run::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .cwd(repo_root)
        .timeout(Duration::from_secs(30))
        .output()
        .map_err(|e| anyhow!("{e}"))?;
    let commit = output.stdout.trim();
    if output.code != 0 || commit.is_empty() {
        bail!("git rev-parse --short HEAD exited with {}", output.code);
    }
    Ok(commit.to_string())
}

/// Make `image` executable and copy it to the stable name beside it; returns the copy's path.
pub(crate) fn finish(image: &Path) -> Result<PathBuf> {
    fs::set_permissions(image, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("marking {} executable", image.display()))?;
    let stable = image.with_file_name(stable_name());
    if stable != image {
        let _ = fs::remove_file(&stable);
        if fs::hard_link(image, &stable).is_err() {
            fs::copy(image, &stable).with_context(|| format!("copying to {}", stable.display()))?;
        }
    }
    Ok(stable)
}

#[cfg(test)]
#[path = "tests/runtime.rs"]
mod tests;
