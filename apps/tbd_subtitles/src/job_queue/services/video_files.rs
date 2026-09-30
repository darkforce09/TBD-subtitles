//! Which files are videos the app can take: by extension, not still downloading, not a localized
//! copy the app wrote, and without a subtitle file beside them, directly in a folder or anywhere
//! under it; and the localized copy a job wrote.
//!
//! **Role:** tell a video from any other file, find the subtitle file beside a video, notice a
//! download still in progress by its part file, list the videos in a folder (its direct
//! children) or under it (every subfolder down to `MAX_DEPTH`), and find the localized video a
//! job's work directory records.
//!
//! **Position:** called by `queue_editing` when a folder is added, by `watch_scan` for each watch
//! folder, by the queue's row menu for a video's subtitle file, and by the job runner and the
//! application when a job ends, for its localized video.
//!
//! **Signals and state:** reads folder listings, file metadata and a job's
//! `visual/localized_video.json`; writes nothing.
//!
//! **Invariants:** a video that already has a subtitle file in any format the app writes is never
//! listed, nor is a `<name>.localized.mkv` the app writes beside a source; the walk under a folder never enters a hidden folder or a symlinked folder, never lists
//! an empty file or a video with a part file beside it, stops `MAX_DEPTH` folders down, and
//! returns its videos sorted.

use std::path::{Path, PathBuf};

use job_model::StepName;
use job_model::job::OutputFormat;
use job_model::onscreen::LocalizedVideoRecord;
use pipeline::work_dir::WorkDir;

/// File extensions the queue takes as videos.
pub(crate) const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "m4v", "mov", "avi", "webm", "ts"];

/// Suffixes a downloader adds to a file it is still writing: "<file name><suffix>" beside a
/// video means the video is not complete yet.
pub(crate) const PARTIAL_SUFFIXES: [&str; 3] = [".part", ".crdownload", ".!qB"];

/// How many folders below the folder it is given `videos_under` descends.
pub(crate) const MAX_DEPTH: usize = 16;

/// Whether `path` names a video, by its extension in any case.
pub(crate) fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Whether a downloader's part file for `video` ("<file name><suffix>") lies beside it.
pub(crate) fn has_partial_sibling(video: &Path) -> bool {
    let Some(name) = video.file_name() else {
        return false;
    };
    PARTIAL_SUFFIXES.iter().any(|suffix| {
        let mut partial = name.to_owned();
        partial.push(suffix);
        video.with_file_name(partial).exists()
    })
}

/// Whether `path` is named like the localized copy the app writes beside a source video,
/// `<name>.localized.mkv`, in any case.
pub(crate) fn is_localized_copy(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|name| name.ends_with(".localized.mkv"))
}

/// The localized video the job in `work_dir` recorded, while the file is there.
pub(crate) fn localized_video(work_dir: &Path) -> Option<PathBuf> {
    let record = WorkDir::new(work_dir).text(StepName::LocalizedVideo);
    let text = std::fs::read_to_string(record).ok()?;
    let record: LocalizedVideoRecord = serde_json::from_str(&text).ok()?;
    record.path.map(PathBuf::from).filter(|path| path.is_file())
}

/// The subtitle file beside `video`, in any format the app writes.
pub(crate) fn subtitle_file(video: &Path) -> Option<PathBuf> {
    OutputFormat::ALL
        .iter()
        .map(|format| video.with_extension(format.extension()))
        .find(|path| path.is_file())
}

/// The videos directly in `folder`, by name, that have no subtitle file beside them yet.
pub(crate) fn videos_in_folder(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut videos: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && is_video(path)
                && !is_localized_copy(path)
                && subtitle_file(path).is_none()
        })
        .collect();
    videos.sort();
    videos
}

/// The complete videos anywhere under `folder`, down to `MAX_DEPTH` folders below it, that have
/// no subtitle file beside them yet, sorted. Hidden entries, symlinked folders, empty files and
/// videos with a part file beside them are left out; a folder that cannot be read is skipped.
pub(crate) fn videos_under(folder: &Path) -> Vec<PathBuf> {
    let mut videos = Vec::new();
    walk(folder, 0, &mut videos);
    videos.sort();
    videos
}

/// Add the videos in `folder`, `depth` folders below the walk's start, and under it.
fn walk(folder: &Path, depth: usize, videos: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) => {
            tracing::debug!(folder = %folder.display(), %error, "skipping a folder it cannot read");
            return;
        }
    };
    for entry in entries.filter_map(Result::ok) {
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(link) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if link.is_dir() {
            if depth < MAX_DEPTH {
                walk(&path, depth + 1, videos);
            }
            continue;
        }
        // A symlinked file counts as the file it points to; a symlinked folder is not entered.
        let Ok(metadata) = std::fs::metadata(&path) else {
            continue;
        };
        if metadata.is_file()
            && metadata.len() > 0
            && is_video(&path)
            && !is_localized_copy(&path)
            && !has_partial_sibling(&path)
            && subtitle_file(&path).is_none()
        {
            videos.push(path);
        }
    }
}

#[cfg(test)]
#[path = "tests/video_files.rs"]
mod tests;
