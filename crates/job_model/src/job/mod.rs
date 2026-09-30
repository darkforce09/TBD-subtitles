//! The job record: one video, its settings, and the fingerprint and measurements of every
//! finished step, as kept in `job.json`.

mod record;
mod settings;

pub use record::{JobRecord, StepMeasure, StepRecord, WorkerMeasure};
pub use settings::{JobSettings, OutputFormat, Separator, WhisperModel};

#[cfg(test)]
#[path = "tests/archive.rs"]
mod archive_tests;
