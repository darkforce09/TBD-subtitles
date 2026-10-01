//! The contracts between the pipeline's stages.
//!
//! **Role:** the types that stages store in a job's database as rkyv archives, and that print as
//! JSON: the job record, the stage names, each stage's output, the on-screen text documents and
//! the job report.
//!
//! **Position:** the bottom layer; every other product crate depends on it, and it depends on no
//! workspace crate.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** a type here changes only together with every stage that reads or writes it;
//! the JSON names stay stable, so `dump` and older JSON read the same names. A change to a
//! type's fields changes its rkyv layout, and nothing detects that by itself: a change that keeps
//! the size passes rkyv's validation. Whoever changes a contract type bumps the layout version of
//! every table that stores it, so the steps that wrote those tables rerun ("Adding a field" in
//! `documentation/architecture/binary_storage_plan.md`).

pub mod job;
pub mod model_call;
pub mod onscreen;
pub mod outputs;
pub mod report;
pub mod stage;
pub mod store;

pub use stage::{StageName, StepName};

#[cfg(test)]
#[path = "tests/archive_round_trip.rs"]
mod archive_round_trip;
