//! What the queue panel asks the application to do.

/// An event the queue panel returns; the application applies it after the frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JobQueueEvent {
    /// Take the video at this queue position out of the queue.
    Remove(usize),
}
