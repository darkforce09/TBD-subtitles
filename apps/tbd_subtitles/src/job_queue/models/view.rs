//! The borrowed views of the queue that the application lends the panels each frame.

use std::time::Instant;

use crate::job_queue::models::progress::Rates;
use crate::job_queue::models::queue::Queue;

/// The queue as the panel sees it: read-only, borrowed for one frame.
pub(crate) struct JobQueueView<'a> {
    pub(crate) queue: &'a Queue,
    /// A model or runtime archive is missing, so no job can start.
    pub(crate) models_missing: bool,
    /// Seconds per second of video for each step, for the time left.
    pub(crate) rates: &'a Rates,
    pub(crate) now: Instant,
}
