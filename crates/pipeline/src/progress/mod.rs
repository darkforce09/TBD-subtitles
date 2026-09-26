//! What a running job reports: which step starts, is skipped, advances or finishes, and the job's
//! end. The command line prints these; the window will show them.

use std::path::PathBuf;

use job_model::StepName;
use job_model::job::StepMeasure;

/// One event of a running job.
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    /// The job's work directory and how many steps it has.
    JobStarted {
        video: PathBuf,
        work_dir: PathBuf,
    },
    /// The step's output is still valid.
    StepSkipped(StepName),
    StepStarted(StepName),
    /// `done` of `total` units (chunks, batches, blocks).
    StepAdvanced {
        step: StepName,
        done: usize,
        total: usize,
    },
    /// A line the step printed.
    StepMessage {
        step: StepName,
        text: String,
    },
    StepFinished {
        step: StepName,
        measure: StepMeasure,
    },
}

/// Where a job's events go; called from the runner's thread and the shot-scan thread.
pub type ProgressSink<'a> = &'a (dyn Fn(Progress) + Sync);
