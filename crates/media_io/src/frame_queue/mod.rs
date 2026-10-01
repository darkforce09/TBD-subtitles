//! A bounded queue between a decoder and whoever reads its frames: the decoder runs on a thread
//! of its own and stays up to `depth` frames ahead, so FFmpeg never waits for the reader's work
//! and the reader never waits for a frame already decoded.
//!
//! **Role:** run any `Producer` on a thread into a bounded channel, hand its items out in order,
//! and report how it ended; and recycle frame buffers through `BufferPool`.
//!
//! **Position:** used by the detection scan, the region source and the localized video around
//! the frame streams of `video_frames`.
//!
//! **Signals and state:** one producer thread per queue; a `sync_channel` of `depth` items.
//!
//! **Invariants:** items arrive in the order produced; at most `depth` items wait in the queue;
//! the producer's `finish` runs exactly once, told whether it produced everything; a reader that
//! stops early drops the queue's receiver, which ends the producer at its next item; a producer's
//! error reaches the reader from `recv`, or from `finish` when the reader had already stopped; a
//! panic on the producer thread comes back from `finish`; none is lost.

mod pool;

use std::sync::mpsc::{Receiver, SendError, SyncSender, sync_channel};
use std::thread::JoinHandle;

pub use pool::{BufferPool, PooledBuffer};

/// Something that yields items one at a time and is closed once at the end.
pub trait Producer: Send + 'static {
    type Item: Send + 'static;
    type Error: Send + 'static;

    /// The next item, or `None` at the end.
    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error>;

    /// Close the producer; `completed` is false when the reader stopped before the end.
    fn finish(self, completed: bool) -> Result<(), Self::Error>;
}

/// A producer's items, read from the queue in order.
pub struct FrameQueue<T, E> {
    items: Option<Receiver<Result<T, E>>>,
    thread: Option<JoinHandle<Result<(), E>>>,
    failed: bool,
}

impl<T: Send + 'static, E: Send + 'static> FrameQueue<T, E> {
    /// Run `producer` on a thread, at most `depth` items ahead of the reader.
    pub fn spawn<P>(producer: P, depth: usize) -> FrameQueue<T, E>
    where
        P: Producer<Item = T, Error = E>,
    {
        let (sender, receiver) = sync_channel(depth.max(1));
        let thread = std::thread::Builder::new()
            .name("frame-queue".to_string())
            .spawn(move || produce(producer, &sender))
            .expect("the frame queue's thread starts");
        FrameQueue {
            items: Some(receiver),
            thread: Some(thread),
            failed: false,
        }
    }

    /// The next item, `Ok(None)` at the end, or the producer's error once.
    pub fn recv(&mut self) -> Result<Option<T>, E> {
        if self.failed {
            return Ok(None);
        }
        match self.items.as_ref().and_then(|items| items.recv().ok()) {
            Some(Ok(item)) => Ok(Some(item)),
            Some(Err(error)) => {
                self.failed = true;
                Err(error)
            }
            None => Ok(None),
        }
    }

    /// Stop reading, wait for the producer, and return how it ended. `panicked` makes the error
    /// for a producer thread that panicked.
    pub fn finish(mut self, panicked: impl FnOnce() -> E) -> Result<(), E> {
        drop(self.items.take());
        match self.thread.take().map(JoinHandle::join) {
            Some(Ok(ended)) => ended,
            Some(Err(_)) => Err(panicked()),
            None => Ok(()),
        }
    }
}

impl<T, E> Drop for FrameQueue<T, E> {
    fn drop(&mut self) {
        drop(self.items.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Send `producer`'s items until it ends, fails or the reader leaves, then close it.
fn produce<P: Producer>(
    mut producer: P,
    sender: &SyncSender<Result<P::Item, P::Error>>,
) -> Result<(), P::Error> {
    loop {
        match producer.next() {
            Ok(Some(item)) => {
                if sender.send(Ok(item)).is_err() {
                    return producer.finish(false);
                }
            }
            Ok(None) => return producer.finish(true),
            Err(error) => {
                let _ = producer.finish(false);
                // An error the reader is no longer there to take comes back from `finish`.
                return match sender.send(Err(error)) {
                    Err(SendError(Err(error))) => Err(error),
                    _ => Ok(()),
                };
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/frame_queue.rs"]
mod tests;
