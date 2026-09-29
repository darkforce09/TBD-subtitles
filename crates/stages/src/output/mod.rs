//! Output: the subtitle file written beside the video, with the same base name, and any file it
//! replaces backed up into the job's work directory.
//!
//! **Role:** the last step; the only code that writes outside the work directory.
//!
//! **Position:** called by the output step in the job runner with the text of the chosen format.
//!
//! **Signals and state:** reads and replaces `<video base name>.<extension>`; copies a replaced
//! file into the backup folder; moves the job's file of another format into it.
//!
//! **Invariants:** the video itself is never opened for writing; the subtitle file is written to
//! a part file and renamed, so VLC never reads half a file; a file that differs from the new one
//! is backed up before it is replaced, and an identical one is left as it is; only a sibling of
//! the video with its base name and a subtitle extension is ever moved aside, and only after
//! the replacement is installed successfully.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use job_model::job::OutputFormat;

/// Where the installed file went, and the backup of what it replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub path: PathBuf,
    pub backup: Option<PathBuf>,
    /// The file already held exactly this text.
    pub unchanged: bool,
    /// Where the job's file of another format was moved, so one subtitle file stays beside the
    /// video.
    pub retired: Option<PathBuf>,
}

/// `<folder>/<base name>.<extension>` for `video` in `format`.
pub fn subtitle_path(video: &Path, format: OutputFormat) -> PathBuf {
    video.with_extension(format.extension())
}

/// Write `text` as the video's subtitle file in `format`; a different existing file is first
/// copied to `backup_dir` as `<file name>.<stamp>`. `earlier` is the subtitle file the job wrote
/// last time: when it is the video's file of another format, it is moved to `backup_dir` after
/// the new subtitle file is installed or confirmed identical.
pub fn install(
    video: &Path,
    format: OutputFormat,
    text: &str,
    backup_dir: &Path,
    stamp: &str,
    earlier: Option<&Path>,
) -> io::Result<Installed> {
    let path = subtitle_path(video, format);
    if path == video {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("the video itself is named .{}", format.extension()),
        ));
    }
    let earlier = earlier.filter(|e| is_other_format(video, &path, e) && e.exists());
    let mut backup = None;
    match fs::read(&path) {
        Ok(existing) if existing == text.as_bytes() => {
            return Ok(Installed {
                path,
                backup: None,
                unchanged: true,
                retired: earlier
                    .map(|old| retire(old, backup_dir, stamp))
                    .transpose()?,
            });
        }
        Ok(_) => {
            fs::create_dir_all(backup_dir)?;
            let target = backup_dir.join(format!("{}.{stamp}", file_name(&path)));
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
        retired: earlier
            .map(|old| retire(old, backup_dir, stamp))
            .transpose()?,
    })
}

/// Whether `earlier` is the video's subtitle file in a format other than the one at `path`.
fn is_other_format(video: &Path, path: &Path, earlier: &Path) -> bool {
    earlier != path
        && OutputFormat::ALL
            .iter()
            .any(|f| subtitle_path(video, *f) == earlier)
}

/// Move `old` into `backup_dir` as `<file name>.<stamp>`.
fn retire(old: &Path, backup_dir: &Path, stamp: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(backup_dir)?;
    let target = backup_dir.join(format!("{}.{stamp}", file_name(old)));
    // A copy and a removal, not a rename: the work directory is often on another disk.
    fs::copy(old, &target)?;
    fs::remove_file(old)?;
    Ok(target)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| "subtitles".into(), |n| n.to_string_lossy().into_owned())
}

#[cfg(test)]
#[path = "tests/output.rs"]
mod tests;
