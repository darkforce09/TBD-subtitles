//! One cap on how many language-model calls run at once, shared by every run that holds a seat.
//!
//! **Role:** admit a call when a slot is free and it is first in line; the line is ordered by
//! seat, then by arrival, so every call of a run started earlier goes before any call of a run
//! started later.
//!
//! **Position:** used through [`Gated`], which wraps a backend so each `complete_json` takes a
//! slot for the call and gives it back after; the window holds one gate for all Fix It runs, and
//! the `fix` subcommand a gate of its own.
//!
//! **Signals and state:** a mutex over the limit, the slots held, the waiting line and each
//! seat's counts, and a condition variable woken on every change. A waiter wakes at least every
//! 100 ms to look at its cancel flag.
//!
//! **Invariants:** no more calls hold a slot than the limit, which is at least 1; a waiter is
//! admitted only at the head of the line; a cancelled waiter leaves the line and takes no slot;
//! lowering the limit stops no running call; a poisoned lock is recovered, never a panic.

mod gated;

pub use gated::{Gated, Retry, is_transient};

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// How long a waiter sleeps before it looks at its cancel flag again.
const SLICE: Duration = Duration::from_millis(100);

/// The shared cap on calls at once.
pub struct CallGate {
    state: Mutex<GateState>,
    changed: Condvar,
}

/// A seat's calls holding a slot and waiting for one.
#[derive(Debug, Default, Clone, Copy)]
struct SeatCounts {
    held: usize,
    waiting: usize,
}

struct GateState {
    /// At least 1.
    limit: usize,
    held: usize,
    /// The line: `(seat order, arrival)`, smallest first.
    waiting: BTreeSet<(u64, u64)>,
    next_seat: u64,
    next_arrival: u64,
    /// Seats with a call holding or waiting; an entry goes once both counts are 0.
    seats: BTreeMap<u64, SeatCounts>,
}

impl GateState {
    fn counts(&mut self, order: u64) -> &mut SeatCounts {
        self.seats.entry(order).or_default()
    }

    fn tidy(&mut self, order: u64) {
        if let Some(counts) = self.seats.get(&order)
            && counts.held == 0
            && counts.waiting == 0
        {
            self.seats.remove(&order);
        }
    }
}

impl CallGate {
    /// A gate that lets `limit` calls run at once (0 counts as 1).
    pub fn new(limit: usize) -> Arc<CallGate> {
        Arc::new(CallGate {
            state: Mutex::new(GateState {
                limit: limit.max(1),
                held: 0,
                waiting: BTreeSet::new(),
                next_seat: 0,
                next_arrival: 0,
                seats: BTreeMap::new(),
            }),
            changed: Condvar::new(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, GateState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Change the cap (0 counts as 1). A raised cap admits waiters at once; a lowered one stops
    /// no running call, and new calls wait until fewer than the cap hold a slot.
    pub fn set_limit(&self, limit: usize) {
        self.lock().limit = limit.max(1);
        self.changed.notify_all();
    }

    pub fn limit(&self) -> usize {
        self.lock().limit
    }

    /// Calls holding a slot now.
    pub fn held(&self) -> usize {
        self.lock().held
    }

    /// Calls waiting for a slot now.
    pub fn waiting(&self) -> usize {
        self.lock().waiting.len()
    }

    /// A seat for one run: its calls go after those of every seat taken earlier.
    pub fn seat(self: &Arc<Self>) -> CallSeat {
        let mut state = self.lock();
        let order = state.next_seat;
        state.next_seat += 1;
        CallSeat {
            gate: self.clone(),
            order,
        }
    }
}

impl fmt::Debug for CallGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.lock();
        f.debug_struct("CallGate")
            .field("limit", &state.limit)
            .field("held", &state.held)
            .field("waiting", &state.waiting.len())
            .finish()
    }
}

/// One run's place at a gate; its clones share the place.
#[derive(Clone)]
pub struct CallSeat {
    gate: Arc<CallGate>,
    order: u64,
}

impl fmt::Debug for CallSeat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallSeat")
            .field("order", &self.order)
            .field("limit", &self.gate.limit())
            .finish()
    }
}

impl CallSeat {
    /// Wait for a slot; `None` once `cancel` is set, before or while waiting.
    pub fn acquire(&self, cancel: &AtomicBool) -> Option<Permit> {
        if cancel.load(Ordering::SeqCst) {
            return None;
        }
        let gate = &self.gate;
        let mut state = gate.lock();
        let key = (self.order, state.next_arrival);
        state.next_arrival += 1;
        state.waiting.insert(key);
        state.counts(self.order).waiting += 1;
        loop {
            if state.held < state.limit && state.waiting.first() == Some(&key) {
                state.waiting.remove(&key);
                state.held += 1;
                let counts = state.counts(self.order);
                counts.waiting -= 1;
                counts.held += 1;
                drop(state);
                // The next in line may fit in a slot too.
                gate.changed.notify_all();
                return Some(Permit {
                    gate: gate.clone(),
                    order: self.order,
                });
            }
            if cancel.load(Ordering::SeqCst) {
                state.waiting.remove(&key);
                state.counts(self.order).waiting -= 1;
                state.tidy(self.order);
                drop(state);
                gate.changed.notify_all();
                return None;
            }
            state = gate
                .changed
                .wait_timeout(state, SLICE)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// Whether this run has a call waiting and none holding a slot: it is only waiting its turn.
    pub fn waiting_only(&self) -> bool {
        let state = self.gate.lock();
        state
            .seats
            .get(&self.order)
            .is_some_and(|counts| counts.waiting > 0 && counts.held == 0)
    }

    /// The gate this seat is at.
    pub fn gate(&self) -> &Arc<CallGate> {
        &self.gate
    }
}

/// A slot held for one call; dropping it frees the slot.
pub struct Permit {
    gate: Arc<CallGate>,
    order: u64,
}

impl Drop for Permit {
    fn drop(&mut self) {
        let mut state = self.gate.lock();
        state.held -= 1;
        state.counts(self.order).held -= 1;
        state.tidy(self.order);
        drop(state);
        self.gate.changed.notify_all();
    }
}

#[cfg(test)]
#[path = "tests/call_gate.rs"]
mod tests;
