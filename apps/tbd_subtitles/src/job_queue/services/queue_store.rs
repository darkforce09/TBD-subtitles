//! The queue kept across windows: `queue.json` in the app data folder, so a long batch survives
//! the window closing.
//!
//! **Role:** write each job's video, kind, coarse state, whether it keeps its own settings, the
//! steps it runs again, its corrections and how it failed with the steps it had finished, and read
//! them back as a queue; give a failure an older window kept its finished steps from `job.json`.
//!
//! **Position:** called by the application after every change of the queue and when the window
//! opens.
//!
//! **Signals and state:** reads and writes `queue.json` (through a part file); reads a failed
//! job's `job.json` in the work folder.
//!
//! **Invariants:** a job running when the window closed waits again; a loaded queue does not run
//! until the owner presses Start; a file written before a field existed still loads, the field
//! taking its default, and a job there that no longer waits keeps its own settings.

use std::path::{Path, PathBuf};

use job_model::StepName;
use job_model::job::JobRecord;
use serde::{Deserialize, Serialize};

use crate::job_queue::models::progress::FinishedStep;
use crate::job_queue::models::queue::{Failure, JobKind, JobState, Queue};
use crate::job_queue::services::queue_editing;

/// One job as the file keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// The steps a failed job had finished; missing from files older than the list.
    #[serde(default)]
    finished: Vec<StoredStep>,
}

/// A step a failed job had finished: done in its seconds, or still valid from an earlier run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredStep {
    step: StepName,
    #[serde(default)]
    seconds: Option<f64>,
    #[serde(default)]
    still_valid: bool,
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
                finished: failure
                    .map(|f| f.finished.iter().map(stored_step).collect())
                    .unwrap_or_default(),
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
                    finished: item.finished.iter().map(finished_step).collect(),
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

/// Give each failure an older window kept, which knows only how many steps it kept, its finished
/// steps: the steps before the failed one that its job's `job.json` under `work_root` records,
/// with their seconds, else the first of them it kept, done in a time not known; it then keeps as
/// many as it lists.
pub(crate) fn read_finished_steps(queue: &mut Queue, work_root: &Path) {
    for item in &mut queue.items {
        let JobState::Failed(failure) = &mut item.state else {
            continue;
        };
        if !failure.finished.is_empty() || failure.kept_steps == 0 {
            continue;
        }
        let at = failure
            .step
            .and_then(|failed| StepName::ALL.iter().position(|&step| step == failed))
            .unwrap_or(StepName::ALL.len());
        let before = &StepName::ALL[..at];
        let record = std::fs::canonicalize(&item.video)
            .ok()
            .map(|video| {
                work_root
                    .join(pipeline::work_dir::job_id(&video))
                    .join("job.json")
            })
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str::<JobRecord>(&text).ok());
        failure.finished = match record {
            Some(record) => before
                .iter()
                .filter_map(|step| {
                    let done = record.steps.get(step)?;
                    Some((*step, FinishedStep::Done(Some(done.measure.wall_s))))
                })
                .collect(),
            None => before
                .iter()
                .take(failure.kept_steps)
                .map(|&step| (step, FinishedStep::Done(None)))
                .collect(),
        };
        failure.kept_steps = failure.finished.len();
    }
}

fn stored_step(&(step, finished): &(StepName, FinishedStep)) -> StoredStep {
    match finished {
        FinishedStep::Done(seconds) => StoredStep {
            step,
            seconds,
            still_valid: false,
        },
        FinishedStep::StillValid => StoredStep {
            step,
            seconds: None,
            still_valid: true,
        },
    }
}

fn finished_step(stored: &StoredStep) -> (StepName, FinishedStep) {
    let finished = if stored.still_valid {
        FinishedStep::StillValid
    } else {
        FinishedStep::Done(stored.seconds)
    };
    (stored.step, finished)
}

#[cfg(test)]
#[path = "tests/queue_store.rs"]
mod tests;
