//! Changing the queue: adding videos, from files or folders, taking jobs out, moving them,
//! retrying them, and choosing the next to run.
//!
//! **Role:** every edit of the queue the owner can make, and the choice of the next job.
//!
//! **Position:** called by the application's queue actions and by `queue_store` when it rebuilds
//! a queue.
//!
//! **Signals and state:** reads a folder's listing when a folder is added; otherwise changes only
//! the queue it is given.
//!
//! **Invariants:** a running job is never removed, moved or retried; a video is never queued
//! twice while it waits or runs; only waiting jobs move, among themselves.

use std::path::{Path, PathBuf};

use job_model::job::OutputFormat;

use crate::job_queue::models::queue::{JobId, JobKind, JobState, Move, Queue, QueueItem};

/// File extensions the queue takes as videos.
const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "m4v", "mov", "avi", "webm", "ts"];

/// Queue each video not already waiting or running, keeping the order given; a folder stands for
/// its videos (`videos_in_folder`). Returns how many were added.
pub(crate) fn add_videos(queue: &mut Queue, videos: impl IntoIterator<Item = PathBuf>) -> usize {
    let mut added = 0;
    for path in videos {
        let expanded = if path.is_dir() {
            videos_in_folder(&path)
        } else {
            vec![path]
        };
        for video in expanded {
            let queued = queue.items.iter().any(|item| {
                item.video == video && (item.state.is_waiting() || item.state.is_running())
            });
            if video.as_os_str().is_empty() || queued {
                continue;
            }
            push(queue, video, JobKind::Full);
            added += 1;
        }
    }
    added
}

/// Queue a run of `kind` for `video` at the end, and return its id.
pub(crate) fn push(queue: &mut Queue, video: PathBuf, kind: JobKind) -> JobId {
    let id = queue.next_id;
    queue.next_id += 1;
    queue.items.push(QueueItem {
        id,
        video,
        kind,
        state: JobState::Waiting,
        ran_before: false,
    });
    id
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

/// Take job `id` out of the queue, unless it is running; returns its video.
pub(crate) fn remove(queue: &mut Queue, id: JobId) -> Option<PathBuf> {
    let at = queue
        .items
        .iter()
        .position(|item| item.id == id && !item.state.is_running())?;
    if queue.selected == Some(id) {
        queue.selected = None;
    }
    Some(queue.items.remove(at).video)
}

/// Move waiting job `id` among the waiting jobs; other jobs keep their places.
pub(crate) fn move_job(queue: &mut Queue, id: JobId, to: Move) {
    let waiting: Vec<usize> = queue
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.state.is_waiting())
        .map(|(i, _)| i)
        .collect();
    let Some(rank) = waiting.iter().position(|&i| queue.items[i].id == id) else {
        return;
    };
    let target = match to {
        Move::Up => rank.checked_sub(1),
        Move::Down => (rank + 1 < waiting.len()).then_some(rank + 1),
        Move::Top => (rank > 0).then_some(0),
    };
    if let Some(target) = target {
        // Moving up, the job goes where the target was; moving down, after the target, whose
        // index fell by one when the job was taken out.
        let item = queue.items.remove(waiting[rank]);
        queue
            .items
            .insert(waiting[target].min(queue.items.len()), item);
    }
}

/// Put a failed, cancelled or finished job back to waiting; `false` when it cannot be retried.
pub(crate) fn retry(queue: &mut Queue, id: JobId) -> bool {
    let Some(item) = queue.get_mut(id) else {
        return false;
    };
    if item.state.is_waiting() || item.state.is_running() {
        return false;
    }
    item.state = JobState::Waiting;
    true
}

/// The first waiting job, which runs next.
pub(crate) fn next_waiting(queue: &Queue) -> Option<JobId> {
    queue
        .items
        .iter()
        .find(|item| item.state.is_waiting())
        .map(|item| item.id)
}

#[cfg(test)]
#[path = "tests/queue_editing.rs"]
mod tests;
