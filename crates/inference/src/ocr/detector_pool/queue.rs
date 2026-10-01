//! The work the pool's session threads share: screening jobs by priority, then confirmations.
//!
//! **Role:** hand each session thread its next task: a waiting screening job, probes first and
//! then by sequence number; once screening ends, the order to close its screening session, and
//! after every screening session is closed, the next confirmation.
//!
//! **Position:** filled by `DetectorPool`, drained by the session threads in `worker.rs`.
//!
//! **Signals and state:** one mutex-guarded state and one condition variable that wakes the
//! threads whenever it changes.
//!
//! **Invariants:** a probe never waits behind a screening job; no confirmation starts while any
//! screening session is still open, so the two phases never hold GPU memory together; every task
//! is handed out once.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

use crate::ocr::pool::{ConfirmJob, Priority, ScreenJob};

/// What a session thread does next.
#[derive(Debug)]
pub enum Task {
    Screen(ScreenJob),
    /// The job's place in the `confirm` call, and the job.
    Confirm(usize, ConfirmJob),
    /// Screening has ended: close the screening session and report it closed.
    CloseScreen,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Screen,
    Confirm,
    Closed,
}

/// A screening job waiting in the heap, ordered so the heap's top is the job to run first.
#[derive(Debug)]
struct Waiting(ScreenJob);

impl Waiting {
    fn key(&self) -> (Priority, u64) {
        (self.0.priority, self.0.seq)
    }
}

impl PartialEq for Waiting {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for Waiting {}

impl PartialOrd for Waiting {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Waiting {
    fn cmp(&self, other: &Self) -> Ordering {
        other.key().cmp(&self.key())
    }
}

/// The queue's state, without the locking.
#[derive(Debug)]
pub struct State {
    screen: BinaryHeap<Waiting>,
    confirm: VecDeque<(usize, ConfirmJob)>,
    phase: Phase,
    screens_open: usize,
}

impl State {
    /// A state for `sessions` threads, each holding an open screening session.
    pub fn new(sessions: usize) -> State {
        State {
            screen: BinaryHeap::new(),
            confirm: VecDeque::new(),
            phase: Phase::Screen,
            screens_open: sessions,
        }
    }

    pub fn push_screen(&mut self, job: ScreenJob) {
        self.screen.push(Waiting(job));
    }

    /// End screening and queue `jobs` behind the closing of every screening session.
    pub fn push_confirm(&mut self, jobs: impl IntoIterator<Item = (usize, ConfirmJob)>) {
        if self.phase == Phase::Screen {
            self.phase = Phase::Confirm;
        }
        self.confirm.extend(jobs);
    }

    /// A screening session has closed.
    pub fn screen_closed(&mut self) {
        self.screens_open = self.screens_open.saturating_sub(1);
    }

    pub fn close(&mut self) {
        self.phase = Phase::Closed;
    }

    /// The next task for a thread that does or does not still hold a screening session, or
    /// `None` while it has to wait.
    pub fn take(&mut self, holds_screen: bool) -> Option<Task> {
        match self.phase {
            Phase::Closed => Some(Task::Exit),
            Phase::Screen => self.screen.pop().map(|Waiting(job)| Task::Screen(job)),
            Phase::Confirm if holds_screen => Some(Task::CloseScreen),
            Phase::Confirm if self.screens_open > 0 => None,
            Phase::Confirm => self
                .confirm
                .pop_front()
                .map(|(index, job)| Task::Confirm(index, job)),
        }
    }
}

/// The shared queue: the state behind a mutex and a condition variable.
#[derive(Debug)]
pub struct Queue {
    state: Mutex<State>,
    changed: Condvar,
}

impl Queue {
    pub fn new(sessions: usize) -> Queue {
        Queue {
            state: Mutex::new(State::new(sessions)),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Change the state and wake every waiting thread.
    pub fn update(&self, change: impl FnOnce(&mut State)) {
        change(&mut self.lock());
        self.changed.notify_all();
    }

    /// The thread's next task; blocks until there is one.
    pub fn next(&self, holds_screen: bool) -> Task {
        let mut state = self.lock();
        loop {
            if let Some(task) = state.take(holds_screen) {
                return task;
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

#[cfg(test)]
#[path = "tests/queue.rs"]
mod tests;
