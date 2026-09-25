//! The job runner.
//!
//! **Role:** runs one job end to end: the stages in graph order, skipping those whose outputs are
//! valid, each GPU stage as a `worker` subprocess of the app binary, with progress events for the
//! GUI and the CLI.
//!
//! **Position:** called by the app's `process` subcommand and GUI; calls `stages`, starts workers
//! through `child_process`, and records state in `job_model` types.
//!
//! **Signals and state:** reads and writes the job's work directory; emits progress events.
//!
//! **Invariants:** one GPU worker runs at a time; a killed job leaves every finished stage valid
//! for resume.

pub mod graph;
pub mod progress;
pub mod resume;
pub mod work_dir;
pub mod workers;
