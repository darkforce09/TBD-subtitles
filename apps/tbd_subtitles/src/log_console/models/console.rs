//! The log window's state: which view shows, the search typed, the activity list and the Model
//! Calls list.
//!
//! **Role:** hold both views of the log window and the search they share, and move to a call
//! from a line that sums it up.
//!
//! **Position:** held by the application; changed by its log-console actions; read by the log
//! window's `ui` through a borrow.
//!
//! **Signals and state:** the view, the search as typed, and the two lists.
//!
//! **Invariants:** one search filters both lists, trimmed and ignoring case; showing a call opens
//! the Model Calls view on it.

use super::activity::Activity;
use super::calls::Calls;

/// The log window's two views.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ConsoleView {
    /// Every line, grouped by video and step.
    #[default]
    Activity,
    /// Every language-model call, with what was sent and what came back.
    Calls,
}

/// The log window's state.
#[derive(Debug, Default)]
pub(crate) struct LogConsole {
    pub(crate) view: ConsoleView,
    search: String,
    pub(crate) activity: Activity,
    pub(crate) calls: Calls,
}

impl LogConsole {
    /// The search field's text as typed.
    pub(crate) fn search(&self) -> &str {
        &self.search
    }

    /// Filter both lists by `search`.
    pub(crate) fn set_search(&mut self, search: String) {
        let lowered = search.trim().to_lowercase();
        self.activity.set_search(lowered.clone());
        self.calls.set_search(lowered);
        self.search = search;
    }

    /// Open the Model Calls view on the call `id`.
    pub(crate) fn show_call(&mut self, id: String) {
        self.view = ConsoleView::Calls;
        self.calls.select(Some(id));
    }
}

#[cfg(test)]
#[path = "tests/console.rs"]
mod tests;
