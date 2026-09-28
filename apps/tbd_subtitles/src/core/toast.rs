//! Short messages at the bottom of the window, each with an optional button.
//!
//! **Role:** hold the toasts shown now: their kind, text, button and when each goes away; at most
//! three, the oldest leaving first.
//!
//! **Position:** owned by the application, which adds one in `apply` and lets them expire in
//! `poll`; drawn by `core::ui::toast`. `A` is what a toast's button does.
//!
//! **Signals and state:** the toasts and the next id; the clock is passed in.
//!
//! **Invariants:** ids are never reused; an expired toast is gone before the next frame draws.

use std::time::{Duration, Instant};

/// A toast's number, unique for the window's life.
pub(crate) type ToastId = u64;

/// How long a toast shows unless its maker says otherwise.
pub(crate) const SHOWN: Duration = Duration::from_millis(4200);
/// How many toasts show at once.
const MOST: usize = 3;

/// What a toast reports; an error is always drawn in red.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToastKind {
    /// Something done.
    Success,
    /// Something to know.
    Info,
    /// Something that has started and is still going.
    Working,
    /// Something that failed.
    Error,
}

/// One toast.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Toast<A> {
    pub(crate) id: ToastId,
    pub(crate) kind: ToastKind,
    pub(crate) text: String,
    /// The button's label and what it does.
    pub(crate) action: Option<(String, A)>,
    /// When it goes away.
    pub(crate) until: Instant,
}

/// The toasts shown now, oldest first.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Toasts<A> {
    shown: Vec<Toast<A>>,
    next_id: ToastId,
}

impl<A> Default for Toasts<A> {
    fn default() -> Toasts<A> {
        Toasts {
            shown: Vec::new(),
            next_id: 0,
        }
    }
}

impl<A> Toasts<A> {
    /// Show `text` for the usual time, with no button; returns its id.
    pub(crate) fn push(
        &mut self,
        kind: ToastKind,
        text: impl Into<String>,
        now: Instant,
    ) -> ToastId {
        self.push_with(kind, text, None, SHOWN, now)
    }

    /// Show `text` for `shown`, with `action` as its button when given; returns its id. The
    /// oldest toast leaves when more than three would show.
    pub(crate) fn push_with(
        &mut self,
        kind: ToastKind,
        text: impl Into<String>,
        action: Option<(String, A)>,
        shown: Duration,
        now: Instant,
    ) -> ToastId {
        let id = self.next_id;
        self.next_id += 1;
        self.shown.push(Toast {
            id,
            kind,
            text: text.into(),
            action,
            until: now + shown,
        });
        if self.shown.len() > MOST {
            self.shown.remove(0);
        }
        id
    }

    /// Take toast `id` away, returning it (its button was pressed).
    pub(crate) fn take(&mut self, id: ToastId) -> Option<Toast<A>> {
        let at = self.shown.iter().position(|toast| toast.id == id)?;
        Some(self.shown.remove(at))
    }

    /// Take away every toast `pick` chooses.
    pub(crate) fn dismiss(&mut self, pick: impl Fn(&Toast<A>) -> bool) {
        self.shown.retain(|toast| !pick(toast));
    }

    /// Take away every toast whose time is up at `now`.
    pub(crate) fn expire(&mut self, now: Instant) {
        self.shown.retain(|toast| toast.until > now);
    }

    /// The toasts shown now, oldest first.
    pub(crate) fn shown(&self) -> &[Toast<A>] {
        &self.shown
    }

    /// When the next toast goes away.
    pub(crate) fn next_expiry(&self) -> Option<Instant> {
        self.shown.iter().map(|toast| toast.until).min()
    }
}

#[cfg(test)]
#[path = "tests/toast.rs"]
mod tests;
