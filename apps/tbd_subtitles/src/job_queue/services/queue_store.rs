//! The queue kept across windows: `queue.json` in the app data folder, so a long batch survives
//! the window closing.
//!
//! **Role:** write each job's video, kind, coarse state, whether it keeps its own settings, the
//! steps it runs again, its corrections and how it failed, and read them back as a queue.
//!
//! **Position:** called by the application after every change of the queue and when the window
//! opens.
//!
//! **Signals and state:** reads and writes `queue.json` (through a part file).
//!
//! **Invariants:** a job running when the window closed waits again; a loaded queue does not run
//! until the owner presses Start; a file written before a field existed still loads, the field
//! taking its default, and a job there that no longer waits keeps its own settings.

use std::path::{Path, PathBuf};

use job_model::StepName;
use serde::{Deserialize, Serialize};

use crate::job_queue::models::queue::{Failure, JobKind, JobState, Queue};
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
    /// Missing from older files: a job that is no longer waiting has started, so it keeps them.
    #[serde(default)]
    keep_settings: Option<bool>,
    #[serde(default)]
    rerun: Vec<StepName>,
    #[serde(default)]
    corrections: usize,
    /// The step a failed job stopped at.
    #[serde(default)]
    failed_step: Option<StepName>,
    /// Why a failed job failed.
    #[serde(default)]
    message: Option<String>,
    /// How many finished steps a failed or cancelled job kept.
    #[serde(default)]
    kept_steps: usize,
}

/// Write the queue's jobs to `path` (through a part file); finished jobs are kept, so their
/// reports stay one click away.
pub(crate) fn save(path: &Path, queue: &Queue) -> Result<(), String> {
    let stored: Vec<Stored> = queue
        .items
        .iter()
        .map(|item| {
            let (state, failure, kept_steps) = match &item.state {
                JobState::Waiting | JobState::Running(_) => ("waiting", None, 0),
                JobState::Finished(_) | JobState::FinishedBefore => ("finished", None, 0),
                JobState::Failed(failure) => ("failed", Some(failure), failure.kept_steps),
                JobState::Cancelled { kept_steps } => ("cancelled", None, *kept_steps),
            };
            Stored {
                video: item.video.clone(),
                state: state.to_string(),
                review: item.kind == JobKind::Review,
                keep_settings: Some(item.keep_settings),
                rerun: item.rerun.clone(),
                corrections: item.corrections,
                failed_step: failure.and_then(|f| f.step),
                message: failure.map(|f| f.message.clone()),
                kept_steps,
            }
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
                "failed" => JobState::Failed(Failure {
                    step: item.failed_step,
                    message: item
                        .message
                        .unwrap_or_else(|| "failed in an earlier window".to_string()),
                    kept_steps: item.kept_steps,
                }),
                "cancelled" => JobState::Cancelled {
                    kept_steps: item.kept_steps,
                },
                "finished" => JobState::FinishedBefore,
                _ => JobState::Waiting,
            };
            job.keep_settings = item.keep_settings.unwrap_or(item.state != "waiting");
            job.rerun = item.rerun;
            job.corrections = item.corrections;
        }
    }
    Ok(queue)
}

#[cfg(test)]
#[path = "tests/queue_store.rs"]
mod tests;
