//! The queue: each video's job, what kind of run it is, and where it stands.
//!
//! **Role:** hold every job in run order with its id, video, kind and state, the selected job,
//! and whether the queue runs.
//!
//! **Position:** owned by the application; changed by `job_queue::services`; drawn by
//! `job_queue::ui`.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** ids are never reused in one window; at most one job runs in each lane, one
//! full run and one review run; a job that has started keeps its own settings.

use std::path::PathBuf;

use job_model::StepName;

use crate::job_queue::models::progress::JobProgress;

/// A job's number in the queue, unique for the window's life.
pub(crate) type JobId = u64;

/// What a run of a job does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JobKind {
    /// Every step still stale, with the settings chosen now.
    Full,
    /// The steps after the owner's corrections, with the job's own settings.
    Review,
}

/// How a finished job came out.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JobResult {
    pub(crate) subtitles: PathBuf,
    pub(crate) work_dir: PathBuf,
    /// Why the job does not pass the quality check; empty when it passes.
    pub(crate) failures: Vec<String>,
    pub(crate) findings: usize,
    pub(crate) wall_s: f64,
}

/// Why and where a job failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Failure {
    /// The step that failed; none when the job failed before its first step.
    pub(crate) step: Option<StepName>,
    pub(crate) message: String,
    /// How many finished steps stay valid, so a retry resumes after them.
    pub(crate) kept_steps: usize,
}

/// Where a job stands.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum JobState {
    Waiting,
    Running(Box<JobProgress>),
    Finished(JobResult),
    /// Finished in an earlier window; its report is read from its work directory when shown.
    FinishedBefore,
    Failed(Failure),
    /// Stopped by the owner; `kept_steps` finished steps stay valid.
    Cancelled {
        kept_steps: usize,
    },
}

impl JobState {
    pub(crate) fn is_waiting(&self) -> bool {
        matches!(self, JobState::Waiting)
    }

    pub(crate) fn is_running(&self) -> bool {
        matches!(self, JobState::Running(_))
    }
}

/// One video's job.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct QueueItem {
    pub(crate) id: JobId,
    pub(crate) video: PathBuf,
    pub(crate) kind: JobKind,
    pub(crate) state: JobState,
    /// Whether the job has started, so every later run of it takes the settings in its own
    /// `job.json` rather than the ones saved now.
    pub(crate) keep_settings: bool,
    /// Steps the next run does again even when their output is valid; the steps after them
    /// follow. Emptied once the pipeline has recorded them in the job's `job.json`; kept when a
    /// run fails before that.
    pub(crate) rerun: Vec<StepName>,
    /// How many corrections the owner saved or took back for this review run; none for a full
    /// run.
    pub(crate) corrections: usize,
}

/// Every job, in run order, and the one the right side shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Queue {
    pub(crate) items: Vec<QueueItem>,
    pub(crate) selected: Option<JobId>,
    /// Whether the queue starts the next waiting job when one ends.
    pub(crate) running: bool,
    pub(crate) next_id: JobId,
}

impl Queue {
    pub(crate) fn get(&self, id: JobId) -> Option<&QueueItem> {
        self.items.iter().find(|item| item.id == id)
    }

    pub(crate) fn get_mut(&mut self, id: JobId) -> Option<&mut QueueItem> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    /// The job running now, if any.
    pub(crate) fn running_job(&self) -> Option<&QueueItem> {
        self.items.iter().find(|item| item.state.is_running())
    }

    pub(crate) fn count(&self, pick: impl Fn(&JobState) -> bool) -> usize {
        self.items.iter().filter(|item| pick(&item.state)).count()
    }
}

/// Where a waiting job moves among the waiting jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Move {
    Up,
    Down,
    Top,
}
