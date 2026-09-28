//! The Model Calls list's copy of the calls: the kept calls, the ones the search lets through,
//! and the call open on the right.
//!
//! **Role:** keep the newest model calls read from the process's log buffer, filter them by the
//! search, and remember which one is open.
//!
//! **Position:** held by `LogConsole`; filled while the log window is open; read by the log
//! window's `ui` through a borrow.
//!
//! **Signals and state:** the calls, the next sequence number to read, the search, the shown
//! positions and the open call's id; changed only through its methods, between frames.
//!
//! **Invariants:** at most `CALL_CAPACITY` calls, oldest first; a search matches the model, the
//! purpose, the video, the step or the id, never the prompt or the answer (too long to search as
//! the owner types); the open call stays open while it is kept, even when the search hides it.

use crate::core::log_buffer::{CALL_CAPACITY, KeptCall};

/// The Model Calls list.
#[derive(Debug, Default)]
pub(crate) struct Calls {
    kept: Vec<KeptCall>,
    next: u64,
    /// Lower-cased and trimmed; empty shows every call.
    search: String,
    shown: Vec<usize>,
    selected: Option<String>,
}

impl Calls {
    /// The sequence number of the first call not read yet.
    pub(crate) fn next(&self) -> u64 {
        self.next
    }

    /// Add calls read from the log buffer, newest last, dropping the oldest past `CALL_CAPACITY`.
    pub(crate) fn append(&mut self, fresh: Vec<KeptCall>) {
        let Some(last) = fresh.last() else {
            return;
        };
        self.next = last.seq + 1;
        let start = self.kept.len();
        self.kept.extend(fresh);
        if self.kept.len() > CALL_CAPACITY {
            let excess = self.kept.len() - CALL_CAPACITY;
            self.kept.drain(..excess);
            self.refilter();
        } else {
            self.admit(start);
        }
    }

    /// Show only the calls holding `search`, already lower-cased and trimmed.
    pub(crate) fn set_search(&mut self, search: String) {
        self.search = search;
        self.refilter();
    }

    /// Forget every kept call; calls made later still arrive.
    pub(crate) fn clear(&mut self) {
        self.kept.clear();
        self.selected = None;
        self.refilter();
    }

    /// Open the call `id` on the right, or none.
    pub(crate) fn select(&mut self, id: Option<String>) {
        self.selected = id;
    }

    /// How many calls are kept.
    pub(crate) fn len(&self) -> usize {
        self.kept.len()
    }

    /// How many calls pass the search.
    pub(crate) fn shown_len(&self) -> usize {
        self.shown.len()
    }

    /// The `row`th call that passes the search.
    pub(crate) fn shown_call(&self, row: usize) -> Option<&KeptCall> {
        self.shown.get(row).and_then(|&at| self.kept.get(at))
    }

    /// The call open on the right, while it is kept.
    pub(crate) fn selected(&self) -> Option<&KeptCall> {
        let id = self.selected.as_deref()?;
        self.kept.iter().find(|kept| kept.call.id == id)
    }

    /// How many kept calls failed.
    pub(crate) fn failed(&self) -> usize {
        self.kept
            .iter()
            .filter(|kept| kept.call.error.is_some())
            .count()
    }

    fn refilter(&mut self) {
        self.shown.clear();
        self.admit(0);
    }

    fn admit(&mut self, start: usize) {
        for at in start..self.kept.len() {
            if self.passes(&self.kept[at]) {
                self.shown.push(at);
            }
        }
    }

    fn passes(&self, kept: &KeptCall) -> bool {
        let holds = |text: &str| text.to_lowercase().contains(&self.search);
        self.search.is_empty()
            || holds(&kept.call.model)
            || holds(&kept.call.purpose)
            || holds(&kept.call.id)
            || kept.video.as_deref().is_some_and(holds)
            || kept.step.as_deref().is_some_and(holds)
    }
}

#[cfg(test)]
#[path = "tests/calls.rs"]
mod tests;
