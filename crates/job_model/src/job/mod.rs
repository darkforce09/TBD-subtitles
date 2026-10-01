//! The job record: one video and its settings, and the fingerprint and measurements of every
//! finished step, as the job database keeps them.

mod record;
mod settings;

pub use record::{JobRecord, JobRun, StepMeasure, StepRecord, StepRecords, WorkerMeasure};
pub use settings::{JobSettings, OutputFormat, Separator, WhisperModel};

#[cfg(test)]
#[path = "tests/archive.rs"]
mod archive_tests;
