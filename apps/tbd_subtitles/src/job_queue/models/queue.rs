//! The queue: each video's job, what kind of run it is, and where it stands.
//!
//! **Role:** hold every job in run order with its id, video, kind and state, the selected job,
//! and whether the queue runs or is pausing; and a row taken out, kept for Undo.
//!
//! **Position:** owned by the application; changed by `job_queue::services`; drawn by
//! `job_queue::ui`.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** ids are never reused in one window; at most one full run runs, and up to four
//! review runs of different videos (one in each review lane); a job that has started keeps its
//! own settings.

use std::path::PathBuf;

use job_model::StepName;

use crate::job_queue::models::progress::{FinishedStep, JobProgress};

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
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Failure {
    /// The step that failed; none when the job failed before its first step.
    pub(crate) step: Option<StepName>,
    pub(crate) message: String,
    /// How many finished steps stay valid, so a retry resumes after them: as many as `finished`
    /// lists, once it is known.
    pub(crate) kept_steps: usize,
    /// The steps that had finished, in run order; empty for a failure an older window kept until
    /// it is read from the job's `job.json`.
    pub(crate) finished: Vec<(StepName, FinishedStep)>,
}

impl Failure {
    /// A failure at `step` with `message`, keeping the steps in `finished`.
    pub(crate) fn new(
        step: Option<StepName>,
        message: String,
        finished: Vec<(StepName, FinishedStep)>,
    ) -> Failure {
        Failure {
            step,
            message,
            kept_steps: finished.len(),
            finished,
        }
    }
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

impl QueueItem {
    /// The video's name as the window shows it: its file name without the extension.
    pub(crate) fn name(&self) -> String {
        self.video.file_stem().map_or_else(
            || self.video.display().to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        )
    }

    /// The name without a leading group tag such as "[Muhn Pace] ", for messages.
    pub(crate) fn short_name(&self) -> String {
        let name = self.name();
        match name
            .strip_prefix('[')
            .and_then(|rest| rest.split_once("] "))
        {
            Some((_, short)) if !short.is_empty() => short.to_string(),
            _ => name,
        }
    }
}

/// Every job, in run order, and the one the right side shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Queue {
    pub(crate) items: Vec<QueueItem>,
    pub(crate) selected: Option<JobId>,
    /// Whether the queue starts the next waiting job when one ends.
    pub(crate) running: bool,
    /// The owner paused the queue while a full run runs: it stops once that run ends.
    pub(crate) pausing: bool,
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
}

/// Where a waiting job moves among the waiting jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Move {
    Up,
    Down,
    Top,
}

/// A row taken out of the queue, kept so Undo can put it back where it was.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Removed {
    /// The row's job first, then the correction runs folded into its row, each with the index it
    /// had in the queue.
    pub(crate) items: Vec<(usize, QueueItem)>,
}

impl Removed {
    /// The removed row's job.
    pub(crate) fn job(&self) -> Option<&QueueItem> {
        self.items.first().map(|(_, item)| item)
    }
}
