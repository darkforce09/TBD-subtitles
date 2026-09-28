//! The contracts between the pipeline's stages.
//!
//! **Role:** the serde types that stages write into a job's work directory and read back: the
//! job record, the stage names, each stage's output and the job report.
//!
//! **Position:** the bottom layer; every other product crate depends on it, and it depends on no
//! workspace crate.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** a type here changes only together with every stage that reads or writes it;
//! the JSON names stay stable so a resumed job reads what an earlier run wrote.

pub mod job;
pub mod model_call;
pub mod outputs;
pub mod report;
pub mod stage;

pub use stage::{StageName, StepName};
