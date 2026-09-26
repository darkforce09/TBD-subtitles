//! Output: the subtitle file written beside the video, with the same base name, and any file it
//! replaces backed up into the job's work directory.
//!
//! **Role:** the last step; the only code that writes outside the work directory.
//!
//! **Position:** called by the output step in the job runner with the SRT text.
//!
//! **Signals and state:** reads and replaces `<video base name>.srt`; copies a replaced file into
//! the backup folder.
//!
//! **Invariants:** the video itself is never opened for writing; the subtitle file is written to
//! a part file and renamed, so VLC never reads half a file; a file that differs from the new one
//! is backed up before it is replaced, and an identical one is left as it is.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Where the installed file went, and the backup of what it replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub path: PathBuf,
    pub backup: Option<PathBuf>,
    /// The file already held exactly this text.
    pub unchanged: bool,
}

/// `<folder>/<base name>.srt` for `video`.
pub fn subtitle_path(video: &Path) -> PathBuf {
    video.with_extension("srt")
}

/// Write `text` as the video's subtitle file; a different existing file is first copied to
/// `backup_dir` as `<file name>.<stamp>`.
pub fn install(video: &Path, text: &str, backup_dir: &Path, stamp: &str) -> io::Result<Installed> {
    let path = subtitle_path(video);
    if path == video {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the video itself is named .srt",
        ));
    }
    let mut backup = None;
    match fs::read(&path) {
        Ok(existing) if existing == text.as_bytes() => {
            return Ok(Installed {
                path,
                backup: None,
                unchanged: true,
            });
        }
        Ok(_) => {
            fs::create_dir_all(backup_dir)?;
            let name = path.file_name().map_or_else(
                || "subtitles.srt".into(),
                |n| n.to_string_lossy().into_owned(),
            );
            let target = backup_dir.join(format!("{name}.{stamp}"));
            fs::copy(&path, &target)?;
            backup = Some(target);
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut part = path.clone().into_os_string();
    part.push(".part");
    let part = PathBuf::from(part);
    fs::write(&part, text)?;
    fs::rename(&part, &path)?;
    Ok(Installed {
        path,
        backup,
        unchanged: false,
    })
}

#[cfg(test)]
#[path = "tests/output.rs"]
mod tests;
