//! The queue logic: editing the queue, running its jobs, following their progress, estimating
//! the time left, and keeping the queue across windows.

pub(crate) mod job_runner;
pub(crate) mod progress_tracking;
pub(crate) mod queue_editing;
pub(crate) mod queue_store;
pub(crate) mod time_left;
