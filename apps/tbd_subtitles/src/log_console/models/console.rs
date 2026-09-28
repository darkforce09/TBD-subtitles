//! The log window's copy of the log, the filter over it, and the lines it lets through.
//!
//! **Role:** keep the newest lines read from the process's log buffer, the level and text the
//! owner filters by, the positions of the lines that pass, and the count of errors and warnings.
//!
//! **Position:** held by the application; filled while the log window is open; read by the log
//! window's `ui` through a borrow.
//!
//! **Signals and state:** the lines, the next sequence number to read, the filter, the shown
//! positions and the two counts; changed only through its methods, between frames.
//!
//! **Invariants:** at most `CAPACITY` lines, oldest first; `shown` always matches the filter over
//! the kept lines; a search matches the target or the message, ignoring case.

use tracing::Level;

use crate::core::log_buffer::{CAPACITY, LogLine};

/// Which lines the window shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConsoleFilter {
    /// The least severe level shown.
    pub(crate) level: Level,
    /// The text a shown line holds, lower-cased; empty shows every line.
    search: String,
}

impl Default for ConsoleFilter {
    fn default() -> ConsoleFilter {
        ConsoleFilter {
            level: Level::DEBUG,
            search: String::new(),
        }
    }
}

impl ConsoleFilter {
    /// Whether `line` passes.
    pub(crate) fn shows(&self, line: &LogLine) -> bool {
        line.level <= self.level
            && (self.search.is_empty()
                || line.message.to_lowercase().contains(&self.search)
                || line.target.to_lowercase().contains(&self.search))
    }
}

/// The log window's lines and filter.
#[derive(Debug, Default)]
pub(crate) struct Console {
    lines: Vec<LogLine>,
    next: u64,
    filter: ConsoleFilter,
    /// The text of the search field as typed.
    search: String,
    shown: Vec<usize>,
    errors: usize,
    warnings: usize,
}

impl Console {
    /// The sequence number of the first line not read yet.
    pub(crate) fn next(&self) -> u64 {
        self.next
    }

    /// Add lines read from the log buffer, newest last, dropping the oldest past `CAPACITY`.
    pub(crate) fn append(&mut self, fresh: Vec<LogLine>) {
        let Some(last) = fresh.last() else {
            return;
        };
        self.next = last.seq + 1;
        let start = self.lines.len();
        self.lines.extend(fresh);
        if self.lines.len() > CAPACITY {
            let excess = self.lines.len() - CAPACITY;
            self.lines.drain(..excess);
            self.refilter();
        } else {
            self.admit(start);
        }
    }

    /// Show only the lines at `level` and more severe.
    pub(crate) fn set_level(&mut self, level: Level) {
        self.filter.level = level;
        self.refilter();
    }

    /// Show only the lines holding `search`.
    pub(crate) fn set_search(&mut self, search: String) {
        self.filter.search = search.trim().to_lowercase();
        self.search = search;
        self.refilter();
    }

    /// Forget every kept line; lines logged later still arrive.
    pub(crate) fn clear(&mut self) {
        self.lines.clear();
        self.refilter();
    }

    pub(crate) fn filter(&self) -> &ConsoleFilter {
        &self.filter
    }

    /// The search field's text as typed.
    pub(crate) fn search(&self) -> &str {
        &self.search
    }

    /// How many lines are kept.
    pub(crate) fn len(&self) -> usize {
        self.lines.len()
    }

    /// How many lines pass the filter.
    pub(crate) fn shown_len(&self) -> usize {
        self.shown.len()
    }

    /// The `row`th line that passes the filter.
    pub(crate) fn shown_line(&self, row: usize) -> Option<&LogLine> {
        self.shown.get(row).and_then(|&at| self.lines.get(at))
    }

    /// Every line that passes the filter, oldest first.
    pub(crate) fn shown(&self) -> impl Iterator<Item = &LogLine> {
        self.shown.iter().filter_map(|&at| self.lines.get(at))
    }

    /// How many kept lines are errors, and how many warnings.
    pub(crate) fn problems(&self) -> (usize, usize) {
        (self.errors, self.warnings)
    }

    fn refilter(&mut self) {
        self.shown.clear();
        self.errors = 0;
        self.warnings = 0;
        self.admit(0);
    }

    /// Count and filter the lines from `start` on.
    fn admit(&mut self, start: usize) {
        for (at, line) in self.lines.iter().enumerate().skip(start) {
            match line.level {
                Level::ERROR => self.errors += 1,
                Level::WARN => self.warnings += 1,
                _ => {}
            }
            if self.filter.shows(line) {
                self.shown.push(at);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/console.rs"]
mod tests;
