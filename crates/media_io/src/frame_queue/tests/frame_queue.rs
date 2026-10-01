use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

/// Counts to `last`, failing at `fail_at`, and records how it was finished.
struct Counter {
    next: u32,
    last: u32,
    fail_at: Option<u32>,
    finished: Arc<AtomicUsize>,
}

const NOT_FINISHED: usize = 0;
const COMPLETED: usize = 1;
const STOPPED_EARLY: usize = 2;

impl Producer for Counter {
    type Item = u32;
    type Error = String;

    fn next(&mut self) -> Result<Option<u32>, String> {
        if Some(self.next) == self.fail_at {
            return Err(format!("failed at {}", self.next));
        }
        if self.next > self.last {
            return Ok(None);
        }
        self.next += 1;
        Ok(Some(self.next - 1))
    }

    fn finish(self, completed: bool) -> Result<(), String> {
        let state = if completed { COMPLETED } else { STOPPED_EARLY };
        self.finished.store(state, Ordering::SeqCst);
        Ok(())
    }
}

fn counter(last: u32, fail_at: Option<u32>) -> (Counter, Arc<AtomicUsize>) {
    let finished = Arc::new(AtomicUsize::new(NOT_FINISHED));
    (
        Counter {
            next: 0,
            last,
            fail_at,
            finished: finished.clone(),
        },
        finished,
    )
}

#[test]
fn items_arrive_in_order_and_the_producer_is_told_it_completed() {
    let (producer, finished) = counter(99, None);
    let mut queue = FrameQueue::spawn(producer, 4);
    let mut seen = Vec::new();
    while let Some(item) = queue.recv().unwrap() {
        seen.push(item);
    }
    assert_eq!(seen, (0..=99).collect::<Vec<_>>());
    queue.finish(|| "panicked".to_string()).unwrap();
    assert_eq!(finished.load(Ordering::SeqCst), COMPLETED);
}

#[test]
fn a_reader_that_stops_early_ends_the_producer() {
    let (producer, finished) = counter(1_000_000, None);
    let mut queue = FrameQueue::spawn(producer, 2);
    assert_eq!(queue.recv().unwrap(), Some(0));
    queue.finish(|| "panicked".to_string()).unwrap();
    assert_eq!(finished.load(Ordering::SeqCst), STOPPED_EARLY);
}

#[test]
fn a_producer_error_reaches_the_reader_once_after_the_items_before_it() {
    let (producer, finished) = counter(10, Some(3));
    let mut queue = FrameQueue::spawn(producer, 8);
    for expected in 0..3 {
        assert_eq!(queue.recv().unwrap(), Some(expected));
    }
    assert_eq!(queue.recv(), Err("failed at 3".to_string()));
    assert_eq!(queue.recv(), Ok(None));
    queue.finish(|| "panicked".to_string()).unwrap();
    assert_eq!(finished.load(Ordering::SeqCst), STOPPED_EARLY);
}

#[test]
fn returned_buffers_are_reused_up_to_the_bound() {
    let pool = BufferPool::new(16, 2);
    let buffers: Vec<PooledBuffer> = (0..3).map(|_| pool.take()).collect();
    assert!(buffers.iter().all(|buffer| buffer.len() == 16));
    drop(buffers);
    assert_eq!(pool.idle(), 2, "only `keep` idle buffers are held");
    let mut reused = pool.take();
    reused[0] = 9;
    assert_eq!(pool.idle(), 1);
    drop(reused);
    assert_eq!(pool.take()[0], 9, "a reused buffer keeps what it last held");
    drop(PooledBuffer::detached(vec![1, 2, 3]));
    assert_eq!(pool.idle(), 2, "a detached buffer never joins a pool");
}
