//! The borrowed views of the queue that the application lends the panels each frame.

use std::collections::HashMap;
use std::time::Instant;

use crate::job_queue::models::progress::Rates;
use crate::job_queue::models::queue::{JobId, Queue};
use crate::job_report::models::summary::RowSummary;

/// The queue as the panel sees it: read-only, borrowed for one frame.
pub(crate) struct JobQueueView<'a> {
    pub(crate) queue: &'a Queue,
    /// A model or runtime archive is missing, so no job can start.
    pub(crate) models_missing: bool,
    /// Seconds per second of video for each step, for the time left.
    pub(crate) rates: &'a Rates,
    /// Each finished job's verdict and lines to check, read from its work directory.
    pub(crate) summaries: &'a HashMap<JobId, RowSummary>,
    pub(crate) now: Instant,
}
