//! A running job's progress: every step it will do, where each stands, and the video's length.
//!
//! **Role:** hold one row per step with its state, and answer which step runs, which one the
//! window names, which failed, and which are finished and so kept.
//!
//! **Position:** held by a running `QueueItem`; changed by `job_queue::services::progress_tracking`;
//! read by the time left, the views and the application when the job ends.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** one row per step, in run order; a step this run does not do is never counted
//! as lost.

use std::path::PathBuf;
use std::time::Instant;

use job_model::StepName;

/// Where one step stands.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum StepState {
    /// Not started yet.
    Pending,
    /// Still valid from an earlier run.
    Skipped,
    Running {
        started: Instant,
        done: usize,
        total: usize,
        /// The last line the step printed.
        message: Option<String>,
    },
    Done {
        wall_s: f64,
    },
    Failed(String),
}

/// A step with a valid output when its job ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FinishedStep {
    /// Done in the run that ended, in these seconds, or in a time not known (a failure an older
    /// window kept, with no `job.json` to read the time from).
    Done(Option<f64>),
    /// Skipped as still valid from an earlier run.
    StillValid,
}

/// One step's row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StepRow {
    pub(crate) step: StepName,
    pub(crate) state: StepState,
    /// Whether this run does the step (it was stale when the job started).
    pub(crate) stale: bool,
}

/// A running job.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JobProgress {
    pub(crate) started: Instant,
    pub(crate) work_dir: Option<PathBuf>,
    pub(crate) duration_s: Option<f64>,
    pub(crate) steps: Vec<StepRow>,
    /// Steps the job's settings leave with nothing to do: they only record that they are off,
    /// so they add no time left.
    pub(crate) idle: Vec<StepName>,
    /// The cancel button was pressed; the job is stopping.
    pub(crate) cancelling: bool,
}

impl JobProgress {
    /// A job that has just started: every step pending, none known stale yet, none idle.
    pub(crate) fn new(started: Instant) -> JobProgress {
        JobProgress {
            started,
            work_dir: None,
            duration_s: None,
            steps: StepName::ALL
                .iter()
                .map(|&step| StepRow {
                    step,
                    state: StepState::Pending,
                    stale: true,
                })
                .collect(),
            idle: Vec::new(),
            cancelling: false,
        }
    }

    pub(crate) fn row_mut(&mut self, step: StepName) -> Option<&mut StepRow> {
        self.steps.iter_mut().find(|row| row.step == step)
    }

    /// The steps with a valid output, in run order: done in this run with their seconds, or
    /// still valid from an earlier one.
    pub(crate) fn finished_steps(&self) -> Vec<(StepName, FinishedStep)> {
        self.steps
            .iter()
            .filter_map(|row| match row.state {
                StepState::Done { wall_s } => Some((row.step, FinishedStep::Done(Some(wall_s)))),
                StepState::Skipped => Some((row.step, FinishedStep::StillValid)),
                StepState::Pending if !row.stale => Some((row.step, FinishedStep::StillValid)),
                StepState::Pending | StepState::Running { .. } | StepState::Failed(_) => None,
            })
            .collect()
    }

    /// The step running now; beside the shot scan, the later step.
    pub(crate) fn current_step(&self) -> Option<StepName> {
        self.steps
            .iter()
            .rev()
            .find(|row| matches!(row.state, StepState::Running { .. }))
            .map(|row| row.step)
    }

    /// The step the window names as at work: the running step, else the last one started, never
    /// the shot scan, which runs in the background; none before the first starts.
    pub(crate) fn shown_step(&self) -> Option<StepName> {
        self.steps
            .iter()
            .rev()
            .filter(|row| row.step != StepName::ShotScan)
            .find(|row| {
                matches!(
                    row.state,
                    StepState::Running { .. } | StepState::Done { .. } | StepState::Failed(_)
                )
            })
            .map(|row| row.step)
    }

    /// The first step that failed, if one did.
    pub(crate) fn failed_step(&self) -> Option<StepName> {
        self.steps
            .iter()
            .find(|row| matches!(row.state, StepState::Failed(_)))
            .map(|row| row.step)
    }
}

/// Seconds each step takes per second of video, measured on earlier jobs.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Rates {
    pub(crate) per_step: std::collections::BTreeMap<StepName, f64>,
}

#[cfg(test)]
#[path = "tests/progress.rs"]
mod tests;
