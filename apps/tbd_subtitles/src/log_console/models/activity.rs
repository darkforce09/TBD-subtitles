//! The activity list's copy of the log: the kept lines, the filter over them, the rows the list
//! shows (a header each time the work moves to another video or step), and the line selected.
//!
//! **Role:** keep the newest lines read from the process's log buffer, the level, writer and text
//! the owner filters by, the rows that pass with their headers, the count of errors and warnings,
//! and which line is open in the detail panel.
//!
//! **Position:** held by `LogConsole`; filled while the log window is open; read by the log
//! window's `ui` through a borrow.
//!
//! **Signals and state:** the lines, the next sequence number to read, the filter, the rows, the
//! two counts and the selected line's number; changed only through its methods, between frames.
//!
//! **Invariants:** at most `CAPACITY` lines, oldest first; `rows` always matches the filter over
//! the kept lines; a header comes before the first shown line of each new (video, step), and a
//! line about no video or step never starts a group; a search matches the text, the source, the
//! video or the step, ignoring case.

use tracing::Level;

use super::who::Who;
use crate::core::log_buffer::{CAPACITY, LogLine};

/// Which lines the list shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActivityFilter {
    /// The least severe level shown.
    pub(crate) level: Level,
    /// The one writer shown; `None` shows every writer.
    pub(crate) who: Option<Who>,
    /// The text a shown line holds, lower-cased; empty shows every line.
    search: String,
}

impl Default for ActivityFilter {
    fn default() -> ActivityFilter {
        ActivityFilter {
            level: Level::DEBUG,
            who: None,
            search: String::new(),
        }
    }
}

impl ActivityFilter {
    /// Whether `line` passes.
    pub(crate) fn shows(&self, line: &LogLine) -> bool {
        let holds = |text: &str| text.to_lowercase().contains(&self.search);
        line.level <= self.level
            && self.who.is_none_or(|who| Who::of(&line.target) == who)
            && (self.search.is_empty()
                || holds(&line.message)
                || holds(&line.target)
                || line.video.as_deref().is_some_and(holds)
                || line.step.as_deref().is_some_and(holds))
    }
}

/// One row of the list: a group's header, or a line; each by the line's place in the kept lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Header(usize),
    Line(usize),
}

/// A row as the list draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowView<'a> {
    /// The work moves to this video and step.
    Header {
        video: Option<&'a str>,
        step: Option<&'a str>,
    },
    Line(&'a LogLine),
}

type Context = (Option<String>, Option<String>);

/// The activity list's lines, filter, rows and selection.
#[derive(Debug, Default)]
pub(crate) struct Activity {
    lines: Vec<LogLine>,
    next: u64,
    filter: ActivityFilter,
    rows: Vec<Row>,
    /// The context of the last group started, while rows are built.
    group: Option<Context>,
    shown: usize,
    errors: usize,
    warnings: usize,
    selected: Option<u64>,
}

impl Activity {
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

    /// Show only the lines `who` wrote, or every line.
    pub(crate) fn set_who(&mut self, who: Option<Who>) {
        self.filter.who = who;
        self.refilter();
    }

    /// Show only the lines holding `search`, already lower-cased and trimmed.
    pub(crate) fn set_search(&mut self, search: String) {
        self.filter.search = search;
        self.refilter();
    }

    /// Forget every kept line; lines logged later still arrive.
    pub(crate) fn clear(&mut self) {
        self.lines.clear();
        self.selected = None;
        self.refilter();
    }

    /// Open the line numbered `seq` in the detail panel, or close it.
    pub(crate) fn select(&mut self, seq: Option<u64>) {
        self.selected = seq;
    }

    pub(crate) fn filter(&self) -> &ActivityFilter {
        &self.filter
    }

    /// How many lines are kept.
    pub(crate) fn len(&self) -> usize {
        self.lines.len()
    }

    /// How many lines pass the filter.
    pub(crate) fn shown_len(&self) -> usize {
        self.shown
    }

    /// How many rows the list has: the lines that pass and their headers.
    pub(crate) fn rows_len(&self) -> usize {
        self.rows.len()
    }

    /// The `index`th row.
    pub(crate) fn row(&self, index: usize) -> Option<RowView<'_>> {
        match *self.rows.get(index)? {
            Row::Header(at) => {
                let line = self.lines.get(at)?;
                Some(RowView::Header {
                    video: line.video.as_deref(),
                    step: line.step.as_deref(),
                })
            }
            Row::Line(at) => self.lines.get(at).map(RowView::Line),
        }
    }

    /// Every line that passes the filter, oldest first.
    pub(crate) fn shown(&self) -> impl Iterator<Item = &LogLine> {
        self.rows.iter().filter_map(|row| match row {
            Row::Line(at) => self.lines.get(*at),
            Row::Header(_) => None,
        })
    }

    /// The line open in the detail panel, while it is kept.
    pub(crate) fn selected(&self) -> Option<&LogLine> {
        let seq = self.selected?;
        let at = self
            .lines
            .binary_search_by_key(&seq, |line| line.seq)
            .ok()?;
        self.lines.get(at)
    }

    /// How many kept lines are errors, and how many warnings.
    pub(crate) fn problems(&self) -> (usize, usize) {
        (self.errors, self.warnings)
    }

    fn refilter(&mut self) {
        self.rows.clear();
        self.group = None;
        self.shown = 0;
        self.errors = 0;
        self.warnings = 0;
        self.admit(0);
    }

    /// Count and filter the lines from `start` on, starting a group at each new context.
    fn admit(&mut self, start: usize) {
        for at in start..self.lines.len() {
            let line = &self.lines[at];
            match line.level {
                Level::ERROR => self.errors += 1,
                Level::WARN => self.warnings += 1,
                _ => {}
            }
            if !self.filter.shows(line) {
                continue;
            }
            if line.video.is_some() || line.step.is_some() {
                let context = (line.video.clone(), line.step.clone());
                if self.group.as_ref() != Some(&context) {
                    self.rows.push(Row::Header(at));
                    self.group = Some(context);
                }
            }
            self.rows.push(Row::Line(at));
            self.shown += 1;
        }
    }
}

#[cfg(test)]
#[path = "tests/activity.rs"]
mod tests;
