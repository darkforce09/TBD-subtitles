//! A running job's progress: every step it will do, where each stands, and the video's length.

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
    /// The cancel button was pressed; the job is stopping.
    pub(crate) cancelling: bool,
}

impl JobProgress {
    /// A job that has just started: every step pending, none known stale yet.
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
            cancelling: false,
        }
    }

    pub(crate) fn row_mut(&mut self, step: StepName) -> Option<&mut StepRow> {
        self.steps.iter_mut().find(|row| row.step == step)
    }
}

/// Seconds each step takes per second of video, measured on earlier jobs.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Rates {
    pub(crate) per_step: std::collections::BTreeMap<StepName, f64>,
}
