//! Every video the app has ever queued, from any source, kept in `queued_videos.json` so a watch
//! folder never queues the same video twice.
//!
//! **Role:** remember each queued video, whether added by hand, from a folder or by a watch
//! folder, and whether it finished, failed or was removed since; read and write that list.
//!
//! **Position:** the application records every video it queues and asks `contains` before a
//! watch folder's video is queued.
//!
//! **Signals and state:** reads and writes `queued_videos.json` in the app data folder (through
//! a part file).
//!
//! **Invariants:** a video once recorded stays recorded; a missing or broken file reads as an
//! empty history, a broken one with a warning; the file is replaced whole, never half written.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::job_queue::models::queue::Queue;

/// The file's name in the app data folder.
const FILE_NAME: &str = "queued_videos.json";

/// Every video the app has queued.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct QueuedHistory {
    videos: BTreeSet<PathBuf>,
}

impl QueuedHistory {
    /// Whether `video` was ever queued.
    pub(crate) fn contains(&self, video: &Path) -> bool {
        self.videos.contains(video)
    }

    /// Record `videos` as queued; returns whether any was not recorded before.
    pub(crate) fn record(&mut self, videos: impl IntoIterator<Item = PathBuf>) -> bool {
        let mut added = false;
        for video in videos {
            added |= self.videos.insert(video);
        }
        added
    }

    /// Record the video of every job in `queue`; returns whether any was not recorded before.
    pub(crate) fn record_queue(&mut self, queue: &Queue) -> bool {
        self.record(queue.items.iter().map(|item| item.video.clone()))
    }
}

/// `queued_videos.json` in the app data folder, when the data folder is known.
pub(crate) fn default_path() -> Option<PathBuf> {
    inference::model_store::app_data_dir()
        .ok()
        .map(|dir| dir.join(FILE_NAME))
}

/// The history kept in `path`; a missing file is an empty history, and so is a broken one, with
/// a warning.
pub(crate) fn load(path: &Path) -> QueuedHistory {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return QueuedHistory::default();
        }
        Err(error) => {
            tracing::warn!(file = %path.display(), %error, "cannot read the queued videos");
            return QueuedHistory::default();
        }
    };
    serde_json::from_str(&text).unwrap_or_else(|error| {
        tracing::warn!(file = %path.display(), %error, "the queued videos file is broken");
        QueuedHistory::default()
    })
}

/// Write `history` to `path` through a part file, creating its folder.
pub(crate) fn save(path: &Path, history: &QueuedHistory) -> anyhow::Result<()> {
    let text = serde_json::to_string_pretty(history).context("writing the queued videos")?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)
            .with_context(|| format!("creating {}", folder.display()))?;
    }
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    std::fs::write(&part, text).with_context(|| format!("writing {}", part.display()))?;
    std::fs::rename(&part, path).with_context(|| format!("replacing {}", path.display()))
}

#[cfg(test)]
#[path = "tests/queued_history.rs"]
mod tests;
