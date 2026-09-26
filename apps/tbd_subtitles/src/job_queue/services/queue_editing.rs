//! Adding videos to the queue, from files or folders, and taking them out.

use std::path::{Path, PathBuf};

use job_model::job::OutputFormat;

/// File extensions the queue takes as videos.
const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "m4v", "mov", "avi", "webm", "ts"];

/// Append each video not already queued, keeping the order given; a folder stands for its
/// videos (`videos_in_folder`). Returns how many were added.
pub(crate) fn add_videos(
    queue: &mut Vec<PathBuf>,
    videos: impl IntoIterator<Item = PathBuf>,
) -> usize {
    let mut added = 0;
    for path in videos {
        let expanded = if path.is_dir() {
            videos_in_folder(&path)
        } else {
            vec![path]
        };
        for video in expanded {
            if video.as_os_str().is_empty() || queue.contains(&video) {
                continue;
            }
            queue.push(video);
            added += 1;
        }
    }
    added
}

/// The videos directly in `folder`, by name, that have no subtitle file beside them yet.
pub(crate) fn videos_in_folder(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut videos: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_video(path) && !has_subtitles(path))
        .collect();
    videos.sort();
    videos
}

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn has_subtitles(video: &Path) -> bool {
    OutputFormat::ALL
        .iter()
        .any(|f| video.with_extension(f.extension()).is_file())
}

/// Take the video at `index` out of the queue; an index past the end changes nothing.
pub(crate) fn remove_video(queue: &mut Vec<PathBuf>, index: usize) -> Option<PathBuf> {
    (index < queue.len()).then(|| queue.remove(index))
}

#[cfg(test)]
#[path = "tests/queue_editing.rs"]
mod tests;
