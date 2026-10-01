//! Recycled frame buffers: a decoder reads each frame into a buffer it takes from the pool, and
//! the buffer goes back when the frame is dropped, so a stream of frames allocates only while the
//! pool fills.
//!
//! **Role:** hand out byte buffers of one size and keep the ones returned, up to a bound.
//!
//! **Position:** used by the frame streams of `video_frames` and by `yuv::YuvFrame`.
//!
//! **Signals and state:** a shared list of idle buffers behind a mutex.
//!
//! **Invariants:** every buffer handed out is exactly the pool's size; at most `keep` idle
//! buffers are held, the rest are freed; a buffer outliving its pool is simply freed.

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, PoisonError, Weak};

/// Buffers of one size, reused.
#[derive(Debug, Clone)]
pub struct BufferPool {
    shared: Arc<Shared>,
}

#[derive(Debug)]
struct Shared {
    bytes: usize,
    keep: usize,
    idle: Mutex<Vec<Vec<u8>>>,
}

impl BufferPool {
    /// A pool of `bytes`-long buffers that keeps at most `keep` idle ones.
    pub fn new(bytes: usize, keep: usize) -> BufferPool {
        BufferPool {
            shared: Arc::new(Shared {
                bytes,
                keep,
                idle: Mutex::new(Vec::new()),
            }),
        }
    }

    /// The size of every buffer.
    pub fn bytes(&self) -> usize {
        self.shared.bytes
    }

    /// An idle buffer, or a new one when none is idle. Its contents are whatever it last held.
    pub fn take(&self) -> PooledBuffer {
        let reused = self
            .shared
            .idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop();
        PooledBuffer {
            data: reused.unwrap_or_else(|| vec![0; self.shared.bytes]),
            home: Arc::downgrade(&self.shared),
        }
    }

    /// How many idle buffers the pool holds now.
    pub fn idle(&self) -> usize {
        self.shared
            .idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

/// A buffer that returns to its pool when dropped.
#[derive(Debug)]
pub struct PooledBuffer {
    data: Vec<u8>,
    home: Weak<Shared>,
}

impl PooledBuffer {
    /// A buffer that belongs to no pool, freed when dropped.
    pub fn detached(data: Vec<u8>) -> PooledBuffer {
        PooledBuffer {
            data,
            home: Weak::new(),
        }
    }
}

impl Deref for PooledBuffer {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.data
    }
}

impl DerefMut for PooledBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        let Some(home) = self.home.upgrade() else {
            return;
        };
        if self.data.len() != home.bytes {
            return;
        }
        let mut idle = home.idle.lock().unwrap_or_else(PoisonError::into_inner);
        if idle.len() < home.keep {
            idle.push(std::mem::take(&mut self.data));
        }
    }
}
