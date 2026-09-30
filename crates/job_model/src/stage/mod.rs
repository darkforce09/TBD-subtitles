//! The names of the pipeline's stages and of the steps they are made of.

mod stage_name;
mod step_name;

pub use stage_name::{ArchivedStageName, StageName, UnknownStage};
pub use step_name::{ArchivedStepName, StepName, UnknownStep};

#[cfg(test)]
#[path = "tests/archive.rs"]
mod archive_tests;
