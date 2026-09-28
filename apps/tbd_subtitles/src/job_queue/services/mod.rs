//! The queue logic: editing the queue, the sidebar's rows and their status lines, running its
//! jobs, following their progress, its stages, estimating the time left, and keeping the queue
//! across windows.

pub(crate) mod job_runner;
pub(crate) mod progress_log;
pub(crate) mod progress_tracking;
pub(crate) mod queue_editing;
pub(crate) mod queue_store;
pub(crate) mod sidebar_rows;
pub(crate) mod stage_progress;
pub(crate) mod status_text;
pub(crate) mod time_left;
