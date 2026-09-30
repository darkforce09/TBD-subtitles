//! The runner's side of the worker channel's data path: a step's inputs sent down the worker's
//! stdin from the job database, and its outputs read from the worker's frames straight into it.
//!
//! **Role:** `inputs::send_inputs` streams stored values to a worker as `Input` frames on a thread
//! of its own; `outputs::StepWrite` keeps each `Output` a worker sends in the step's one write
//! transaction and commits it with the step's record.
//!
//! **Position:** used by `workers::run_worker` and `workers::frames::read_frames`; the runner
//! commits a finished step's `StepWrite`; uses `work_dir::store` for the rows and their kinds and
//! `worker_channel` for the frame layout.
//!
//! **Signals and state:** the input thread holds one read snapshot while it writes; a `StepWrite`
//! holds the database's one write transaction from its first output until it commits or drops.
//!
//! **Invariants:** an output is read from the pipe into the row redb reserves for it, with no
//! buffer between, and checked before the next frame is read; nothing a step's worker sent is
//! visible until the runner commits it with the step's record, in one transaction.

pub(crate) mod inputs;
pub mod outputs;

pub use outputs::StepWrite;

#[cfg(test)]
#[path = "tests/channel.rs"]
mod tests;
