//! One attempt to open a database, what it read, and a summary of many attempts.
//!
//! **Role:** the `attempt` command and the attempts the matrix makes in its own process: one
//! open, the repair callbacks it ran, and the counter row it read, rendered as one line.
//!
//! **Position:** used by `main` and `matrix`; depends on `open_mode` and `redb`.
//!
//! **Signals and state:** none; the handle is dropped before the attempt returns.
//!
//! **Invariants:** an attempt never writes; a counter that cannot be read is reported with the
//! error text, never as a number.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use redb::{ReadableDatabase, ReadableTable, TableError};

use crate::open_mode::{COUNTER_KEY, COUNTER_TABLE, Handle, OpenMode, OpenOutcome, Sharing, open};

/// What an open handle found in the counter row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CounterRead {
    /// The row holds this value.
    Value(u64),
    /// The table or the row is missing.
    Missing,
    /// Reading failed with this error text.
    Failed(String),
}

impl CounterRead {
    /// `counter=<n>`, `counter=missing` or `counter unreadable: <error>`.
    pub(crate) fn render(&self) -> String {
        match self {
            CounterRead::Value(value) => format!("counter={value}"),
            CounterRead::Missing => "counter=missing".to_string(),
            CounterRead::Failed(error) => format!("counter unreadable: {error}"),
        }
    }
}

/// One open of a database and what it read.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Attempt {
    pub(crate) outcome: OpenOutcome,
    /// How often redb called the repair callback, when the attempt watched for repair.
    pub(crate) repair_calls: Option<usize>,
    /// The counter row, when the open succeeded.
    pub(crate) counter: Option<CounterRead>,
}

impl Attempt {
    /// One line: the outcome, then the repair and the counter when there are any.
    pub(crate) fn render(&self) -> String {
        let mut line = self.outcome.render();
        if let Some(calls) = self.repair_calls {
            match calls {
                0 => line.push_str("; no repair"),
                calls => line.push_str(&format!("; repair ran ({calls} callback calls)")),
            }
        }
        if let Some(counter) = &self.counter {
            line.push_str("; ");
            line.push_str(&counter.render());
        }
        line
    }
}

/// Opens `path` once, reads the counter row when the open succeeded, and closes it again.
pub(crate) fn attempt(
    path: &Path,
    mode: OpenMode,
    sharing: Sharing,
    watch_repair: bool,
) -> Attempt {
    let calls = watch_repair.then(|| Arc::new(AtomicUsize::new(0)));
    let (outcome, handle) = open(path, mode, sharing, calls.clone());
    let counter = handle.map(|handle| match &handle {
        Handle::ReadWrite(db) => read_counter(db),
        Handle::ReadOnly(db) => read_counter(db),
    });
    Attempt {
        outcome,
        repair_calls: calls.map(|calls| calls.load(Ordering::SeqCst)),
        counter,
    }
}

/// The counter row as a read transaction of `db` sees it.
pub(crate) fn read_counter(db: &impl ReadableDatabase) -> CounterRead {
    let transaction = match db.begin_read() {
        Ok(transaction) => transaction,
        Err(error) => return CounterRead::Failed(format!("{error} | {error:?}")),
    };
    let table = match transaction.open_table(COUNTER_TABLE) {
        Ok(table) => table,
        Err(TableError::TableDoesNotExist(_)) => return CounterRead::Missing,
        Err(error) => return CounterRead::Failed(format!("{error} | {error:?}")),
    };
    match ReadableTable::get(&table, COUNTER_KEY) {
        Ok(Some(value)) => CounterRead::Value(value.value()),
        Ok(None) => CounterRead::Missing,
        Err(error) => CounterRead::Failed(format!("{error} | {error:?}")),
    }
}

/// Many attempts in one line: how many opened and failed, the range of open times, the range
/// of counters read, and each distinct error text with its count.
pub(crate) fn summarize(attempts: &[Attempt]) -> String {
    let total = attempts.len();
    let opened = attempts
        .iter()
        .filter(|attempt| matches!(attempt.outcome, OpenOutcome::Opened { .. }))
        .count();
    let mut line = format!("opened {opened}/{total}, failed {}/{total}", total - opened);
    let times: Vec<f64> = attempts
        .iter()
        .map(|attempt| match attempt.outcome {
            OpenOutcome::Opened { ms } | OpenOutcome::Failed { ms, .. } => ms,
        })
        .collect();
    if let (Some(low), Some(high)) = (
        times.iter().copied().reduce(f64::min),
        times.iter().copied().reduce(f64::max),
    ) {
        line.push_str(&format!("; open took {low:.3}–{high:.3} ms"));
    }
    let counters: Vec<u64> = attempts
        .iter()
        .filter_map(|attempt| match attempt.counter {
            Some(CounterRead::Value(value)) => Some(value),
            _ => None,
        })
        .collect();
    if let (Some(low), Some(high)) = (counters.iter().min(), counters.iter().max()) {
        line.push_str(&format!("; counters read {low}–{high}"));
    }
    let mut other_reads: BTreeMap<String, usize> = BTreeMap::new();
    for attempt in attempts {
        if let Some(read @ (CounterRead::Missing | CounterRead::Failed(_))) = &attempt.counter {
            *other_reads.entry(read.render()).or_default() += 1;
        }
    }
    for (read, count) in other_reads {
        line.push_str(&format!("; {read} ×{count}"));
    }
    let mut errors: BTreeMap<String, usize> = BTreeMap::new();
    for attempt in attempts {
        if let Some(error) = attempt.outcome.error_text() {
            *errors.entry(error).or_default() += 1;
        }
    }
    for (error, count) in errors {
        line.push_str(&format!("; error ×{count}: {error}"));
    }
    line
}

#[cfg(test)]
#[path = "tests/attempt.rs"]
mod tests;
