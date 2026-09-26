//! The queue kept across windows: `queue.json` in the app data folder, so a long batch survives
//! the window closing.
//!
//! **Role:** write each job's video, kind and coarse state, and read them back as a queue.
//!
//! **Position:** called by the application after every change of the queue and when the window
//! opens.
//!
//! **Signals and state:** reads and writes `queue.json` (through a part file).
//!
//! **Invariants:** a job running when the window closed waits again; a loaded queue does not run
//! until the owner presses Start.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::job_queue::models::queue::{JobKind, JobState, Queue};
use crate::job_queue::services::queue_editing;

/// One job as the file keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Stored {
    video: PathBuf,
    /// `waiting`, `finished`, `failed` or `cancelled`; a job running when the window closed is
    /// waiting again.
    state: String,
    #[serde(default)]
    review: bool,
}

/// Write the queue's jobs to `path` (through a part file); finished jobs are kept, so their
/// reports stay one click away.
pub(crate) fn save(path: &Path, queue: &Queue) -> Result<(), String> {
    let stored: Vec<Stored> = queue
        .items
        .iter()
        .map(|item| Stored {
            video: item.video.clone(),
            state: match item.state {
                JobState::Waiting | JobState::Running(_) => "waiting",
                JobState::Finished(_) | JobState::FinishedBefore => "finished",
                JobState::Failed(_) => "failed",
                JobState::Cancelled => "cancelled",
            }
            .to_string(),
            review: item.kind == JobKind::Review,
        })
        .collect();
    let text = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    std::fs::write(&part, text).map_err(|e| e.to_string())?;
    std::fs::rename(&part, path).map_err(|e| e.to_string())
}

/// The queue kept in `path`: waiting jobs wait again; a finished, failed or cancelled job comes
/// back as it was, with its report read from its work directory when shown. No file is an empty
/// queue; a broken one is an error.
pub(crate) fn load(path: &Path) -> Result<Queue, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Queue::default()),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let stored: Vec<Stored> = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not a queue: {e}", path.display()))?;
    let mut queue = Queue::default();
    for item in stored {
        let kind = if item.review {
            JobKind::Review
        } else {
            JobKind::Full
        };
        let id = queue_editing::push(&mut queue, item.video, kind);
        if let Some(job) = queue.get_mut(id) {
            job.state = match item.state.as_str() {
                "failed" => JobState::Failed("failed in an earlier window".to_string()),
                "cancelled" => JobState::Cancelled,
                "finished" => JobState::FinishedBefore,
                _ => JobState::Waiting,
            };
        }
    }
    Ok(queue)
}

#[cfg(test)]
#[path = "tests/queue_store.rs"]
mod tests;
