//! The static FFmpeg 8.1 build the AppImage carries, and the check that it does what the app needs.
//!
//! **Role:** download the pinned BtbN FFmpeg 8.1 linux64 GPL build (hash-checked, cached),
//! copy its `ffmpeg` and `ffprobe` to `usr/bin/ffmpeg/`, and run them to confirm the version,
//! the `scdet` and `apad` filters and the `pulse` output device the clip player uses.
//!
//! **Position:** called by `main`; uses `inference::model_store` for the download and unpack and
//! `child_process` to run the copied programs.
//!
//! **Signals and state:** fills `target/appimage/cache/`; writes the AppDir; runs the bundled
//! `ffmpeg` and `ffprobe`.
//!
//! **Invariants:** the archive's SHA-256 matches the pin before anything is unpacked; a build
//! older than 8.1 or missing a filter or the pulse device fails the run.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use child_process::Run;
use inference::model_store::{self, PinnedArchive};

use crate::gpu_runtime::print_progress;

/// BtbN's static FFmpeg n8.1.3 linux64 GPL build of 2026-09-26: linked statically apart from
/// glibc, with libpulse. The hash is the digest of the downloaded archive.
const FFMPEG_ARCHIVE: PinnedArchive = PinnedArchive {
    id: "ffmpeg-n8.1.3",
    unpack_to: "ffmpeg-n8.1.3",
    url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-09-26-13-03/ffmpeg-n8.1.3-linux64-gpl-8.1.tar.xz",
    size: 150_325_156,
    sha256: "add729675e76163d04ba7e5b7e09c2439a20f66a4eaa23994b20d63f8ad75cb7",
};

/// The two programs the app runs.
const PROGRAMS: [&str; 2] = ["ffmpeg", "ffprobe"];

/// The oldest FFmpeg the app supports.
const MINIMUM_VERSION: (u32, u32) = (8, 1);

/// The filters the pipeline uses: shot changes and audio padding.
const REQUIRED_FILTERS: [&str; 2] = ["scdet", "apad"];

/// The output device the clip player plays through.
const REQUIRED_DEVICE: &str = "pulse";

/// How long one `-version`, `-filters` or `-devices` run may take.
const PROBE_DEADLINE: Duration = Duration::from_secs(30);

/// The unpacked archive's `bin/` folder under `cache`, downloading it when needed.
pub(crate) fn fetch(cache: &Path) -> Result<PathBuf> {
    if !model_store::is_archive_installed(&FFMPEG_ARCHIVE, cache) {
        eprintln!("  downloading {}", FFMPEG_ARCHIVE.url);
        let mut progress = print_progress(FFMPEG_ARCHIVE.id);
        model_store::install_archive(&FFMPEG_ARCHIVE, cache, &mut progress)
            .map_err(|e| anyhow!("installing FFmpeg: {e}"))?;
        eprintln!();
    }
    Ok(cache.join(FFMPEG_ARCHIVE.unpack_to).join("bin"))
}

/// Copy `ffmpeg` and `ffprobe` from `bin` into `<usr_bin>/ffmpeg/`; returns that folder.
pub(crate) fn bundle(bin: &Path, usr_bin: &Path) -> Result<PathBuf> {
    let target = usr_bin.join("ffmpeg");
    fs::create_dir_all(&target).with_context(|| format!("creating {}", target.display()))?;
    for program in PROGRAMS {
        fs::copy(bin.join(program), target.join(program))
            .with_context(|| format!("copying {program} from {}", bin.display()))?;
    }
    Ok(target)
}

/// Run the bundled programs and check the version, the filters and the pulse device; returns
/// the version line.
pub(crate) fn verify(ffmpeg_dir: &Path) -> Result<String> {
    let ffmpeg = ffmpeg_dir.join("ffmpeg");
    let version = probe(&ffmpeg, "-version")?;
    let first = version.lines().next().unwrap_or_default().to_string();
    let (major, minor) =
        parse_version(&first).ok_or_else(|| anyhow!("unreadable FFmpeg version: {first}"))?;
    if (major, minor) < MINIMUM_VERSION {
        bail!("bundled FFmpeg is {major}.{minor}, older than 8.1");
    }
    probe(&ffmpeg_dir.join("ffprobe"), "-version")?;
    let filters = probe(&ffmpeg, "-filters")?;
    for filter in REQUIRED_FILTERS {
        if !lists(&filters, filter) {
            bail!("bundled FFmpeg lacks the {filter} filter");
        }
    }
    if !lists(&probe(&ffmpeg, "-devices")?, REQUIRED_DEVICE) {
        bail!("bundled FFmpeg lacks the {REQUIRED_DEVICE} output device");
    }
    Ok(first)
}

/// Run `program -hide_banner <flag>` and return its stdout.
fn probe(program: &Path, flag: &str) -> Result<String> {
    let output = Run::new(program)
        .args(["-hide_banner", flag])
        .timeout(PROBE_DEADLINE)
        .output()
        .map_err(|e| anyhow!("{e}"))?;
    if output.code != 0 {
        bail!("{} {flag} exited with {}", program.display(), output.code);
    }
    Ok(output.stdout)
}

/// The major and minor version from `ffmpeg version n8.1.3-20260926 …`.
pub(crate) fn parse_version(line: &str) -> Option<(u32, u32)> {
    let word = line.split_whitespace().nth(2)?;
    let digits = word.trim_start_matches(|c: char| !c.is_ascii_digit());
    let mut parts = digits.split(|c: char| !c.is_ascii_digit());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some((major, minor))
}

/// Whether a `-filters` or `-devices` listing has a row naming `name`.
pub(crate) fn lists(listing: &str, name: &str) -> bool {
    listing
        .lines()
        .any(|line| line.split_whitespace().nth(1) == Some(name))
}

#[cfg(test)]
#[path = "tests/ffmpeg.rs"]
mod tests;
