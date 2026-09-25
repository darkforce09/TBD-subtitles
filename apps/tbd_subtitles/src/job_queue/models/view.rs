//! The borrowed view of the queue that the application lends the panel each frame.

use std::path::PathBuf;

/// The queue as the panel sees it: read-only, borrowed for one frame.
pub(crate) struct JobQueueView<'a> {
    /// The queued videos, in run order.
    pub(crate) videos: &'a [PathBuf],
}
