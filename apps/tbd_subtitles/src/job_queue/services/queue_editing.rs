//! Adding videos to the queue and taking them out.

use std::path::PathBuf;

/// Append each video not already queued, keeping the order given; returns how many were added.
pub(crate) fn add_videos(
    queue: &mut Vec<PathBuf>,
    videos: impl IntoIterator<Item = PathBuf>,
) -> usize {
    let mut added = 0;
    for video in videos {
        if video.as_os_str().is_empty() || queue.contains(&video) {
            continue;
        }
        queue.push(video);
        added += 1;
    }
    added
}

/// Take the video at `index` out of the queue; an index past the end changes nothing.
pub(crate) fn remove_video(queue: &mut Vec<PathBuf>, index: usize) -> Option<PathBuf> {
    (index < queue.len()).then(|| queue.remove(index))
}

#[cfg(test)]
#[path = "tests/queue_editing.rs"]
mod tests;
