//! The per-frame rows a worker receives after its documents, read off its stdin one at a time.
//!
//! **Role:** hand a task every row of one per-frame table the runner sends, in key order, each
//! read into one buffer and dropped before the next is read; drain whatever the task did not read
//! so the runner's sending thread ends cleanly.
//! **Position:** held by `tasks::StepIo` in a worker; fed by `worker_channel::worker::read_input`
//! over the stdin `workers::channel::inputs::send_inputs` writes, after
//! `worker_channel::worker::read_documents` took the documents and the first row.
//! **Signals and state:** the reader, the one row read ahead (the first row of a table not yet
//! asked for), and the tables already read.
//! **Invariants:** a table's rows are read once; the rows of the tables the runner sends arrive in
//! `graph::reads_rows` order, so asking for a later table skips the rows of an earlier one never
//! asked for; a row whose key is a name, or whose table the runner does not send, is an error;
//! one row is held at a time.

use std::io::Read;

use worker_channel::address::{Key, Table};
use worker_channel::worker::{Input, read_input};

use crate::error::{Context, PipelineError, Result};

/// The rows still to come on a worker's stdin.
pub(crate) struct RowStream {
    /// The tables the runner sends rows of, in the order it sends them.
    tables: Vec<Table>,
    /// A row read past the end of the table being read; the first row before any table is read.
    ahead: Option<Input>,
    reader: Box<dyn Read + Send>,
    /// The tables whose rows were read.
    taken: Vec<Table>,
}

impl RowStream {
    /// The rows of `tables` on `reader`, the first of them `first` when it was read already.
    pub(crate) fn new(
        tables: Vec<Table>,
        first: Option<Input>,
        reader: Box<dyn Read + Send>,
    ) -> RowStream {
        RowStream {
            tables,
            ahead: first,
            reader,
            taken: Vec::new(),
        }
    }

    /// `f` of every row of `table` as its occurrence, its frame and its bytes, in key order.
    pub(crate) fn each(
        &mut self,
        table: Table,
        mut f: impl FnMut(&str, u64, &[u8]) -> Result<()>,
    ) -> Result<()> {
        let at = format!("the {table} rows");
        let Some(wanted) = self.tables.iter().position(|t| *t == table) else {
            return Err(PipelineError::new(
                at,
                "the runner sends no rows of this table",
            ));
        };
        if self.taken.contains(&table) {
            return Err(PipelineError::new(at, "a worker reads a table's rows once"));
        }
        self.taken.push(table);
        while let Some((address, archive)) = self.next()? {
            let position = self.tables.iter().position(|t| *t == address.table);
            match position {
                Some(p) if p < wanted => continue,
                Some(p) if p > wanted => {
                    self.ahead = Some((address, archive));
                    break;
                }
                Some(_) => {}
                None => {
                    return Err(PipelineError::new(
                        at,
                        format!("a row of the {} table arrived", address.table),
                    ));
                }
            }
            let Key::Frame { occurrence, frame } = &address.key else {
                return Err(PipelineError::new(
                    at,
                    "a named value arrived among the rows",
                ));
            };
            f(occurrence, *frame, &archive)?;
        }
        Ok(())
    }

    /// Read and drop every row still on the pipe, up to its end.
    pub(crate) fn drain(&mut self) -> Result<()> {
        self.ahead = None;
        while self.next()?.is_some() {}
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Input>> {
        match self.ahead.take() {
            Some(row) => Ok(Some(row)),
            None => read_input(&mut self.reader).context("the rows the runner sent do not read"),
        }
    }
}

#[cfg(test)]
#[path = "tests/rows.rs"]
mod tests;
