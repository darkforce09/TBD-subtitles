//! What a running job reports: which step starts, is skipped, advances or finishes, the job's
//! end, and each language-model call a worker made. The command line prints these; the window
//! shows them.

use std::path::PathBuf;

use job_model::StepName;
use job_model::job::StepMeasure;
use job_model::model_call::ModelExchange;

/// One event of a running job.
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    /// The job's work directory and the steps this run will do, in order.
    JobStarted {
        video: PathBuf,
        work_dir: PathBuf,
        stale: Vec<StepName>,
    },
    /// The video's length in seconds, once the probe is there.
    JobDuration(f64),
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
    /// The step stopped with an error, or was cancelled; the job ends with that error.
    StepFailed {
        step: StepName,
        message: String,
    },
    /// A language-model call a worker made, with everything sent and received, for the log window.
    ModelCall {
        step: StepName,
        call: Box<ModelExchange>,
    },
}

/// Where a job's events go; called from the runner's thread and the shot-scan thread.
pub type ProgressSink<'a> = &'a (dyn Fn(Progress) + Sync);
