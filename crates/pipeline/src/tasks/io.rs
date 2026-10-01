//! A task's stored inputs and outputs: what it reads from the job database and what it writes to
//! it, the same calls in the job runner and in a worker.
//!
//! **Role:** `StepIo::get` reads a document of a step the task reads, checked and deserialised;
//! `StepIo::put` keeps a document the task writes, `StepIo::put_frame` a per-frame row, and
//! `StepIo::frame_rows` walks a per-frame table the step reads. In the runner the reads come from
//! one snapshot of the job's store and the writes go into the step's pending write transaction; in
//! a worker the documents come from the `Input` frames the runner sent down stdin, the per-frame
//! rows after them off the same pipe one at a time, and the writes go up the worker channel as
//! `Output` frames.
//!
//! **Position:** made by `tasks::in_process` (from the runner's `JobStore`) and by
//! `tasks::worker_main` (`StepIo::over_stdin`, over `worker_channel::worker::read_documents` and
//! `tasks::rows`); handed to every task; uses `work_dir::store::keys` for the addresses and
//! `workers::StepWrite` for the pending write.
//!
//! **Signals and state:** in process, a read snapshot and, from the first write on, the
//! database's one write transaction, which the runner commits with the step's record; in a
//! worker, the received documents, held until the task ends, and the rows still on stdin, one held
//! at a time.
//!
//! **Invariants:** a document the task reads and does not find is an error naming its key, never
//! a default; every value read is checked by rkyv before use; nothing a task writes is visible to
//! any other reader until the runner commits it with the step's record; a key with no record kind
//! is refused before anything is written; a per-frame table is never held whole.

use std::collections::HashMap;
use std::io::Read;
#[cfg(test)]
use std::io::Write;
use std::sync::Arc;

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::onscreen::TextCorrections;
use job_model::outputs::{Corrections, ProbeDecoded};
use rkyv::api::high::{HighDeserializer, HighSerializer, HighValidator};
use rkyv::bytecheck::CheckBytes;
use rkyv::rancor::Error as ArchiveError;
use rkyv::ser::allocator::ArenaHandle;
use rkyv::util::AlignedVec;
use rkyv::{Archive, Deserialize, Serialize};
use worker_channel::address::{Address, Key, Table};
use worker_channel::frame::{Tag, write_frame};
use worker_channel::worker::Input;

use super::rows::RowStream;
use crate::error::{Context, PipelineError, Result};
use crate::graph;
use crate::work_dir::JobStore;
use crate::work_dir::store::{StoreRead, keys, kinds};
use crate::workers::StepWrite;

/// Where a task's reads come from.
enum Source {
    /// A snapshot of the job's store, in the runner.
    Store(StoreRead),
    /// The archives the runner sent down a worker's stdin, by address.
    Received(HashMap<Address, Vec<u8>>),
}

/// Where a task's writes go.
enum Sink {
    /// The step's pending write, in the runner; the runner commits it with the step's record.
    Store(Box<StepWrite>),
    /// `Output` frames on the worker channel.
    Channel,
    /// `Output` frames on a pipe of a test.
    #[cfg(test)]
    Pipe(Box<dyn Write + Send>),
}

/// A task's stored inputs and outputs.
pub struct StepIo {
    source: Source,
    sink: Sink,
    /// In a worker, the per-frame rows still to come on its stdin.
    rows: Option<RowStream>,
}

impl StepIo {
    /// The inputs and outputs of a step that runs in this process on `store`'s job.
    pub fn in_process(store: &Arc<JobStore>) -> Result<StepIo> {
        Ok(StepIo {
            source: Source::Store(store.read()?),
            sink: Sink::Store(Box::new(StepWrite::new(store.clone()))),
            rows: None,
        })
    }

    /// The inputs of `step`'s worker on `stdin`: the documents, read now, then the rows of the
    /// per-frame tables the step reads, left on the pipe for [`StepIo::frame_rows`]; its outputs
    /// go up the worker channel.
    pub fn over_stdin(step: StepName, mut stdin: impl Read + Send + 'static) -> Result<StepIo> {
        let (documents, first) = worker_channel::worker::read_documents(&mut stdin)
            .context(format!("worker {step}: cannot read the step's inputs"))?;
        Ok(StepIo {
            source: Source::Received(documents.into_iter().collect()),
            sink: Sink::Channel,
            rows: Some(RowStream::new(
                graph::reads_rows(step).to_vec(),
                first,
                Box::new(stdin),
            )),
        })
    }

    /// The inputs a worker received, per-frame rows among them in key order, and its outputs up
    /// the worker channel.
    pub fn in_worker(inputs: Vec<Input>) -> StepIo {
        let (source, rows) = received(inputs);
        StepIo {
            source,
            sink: Sink::Channel,
            rows: Some(rows),
        }
    }

    /// The inputs a test's worker received and its outputs as frames on `out`.
    #[cfg(test)]
    pub(crate) fn on_pipe(inputs: Vec<Input>, out: impl Write + Send + 'static) -> StepIo {
        let (source, rows) = received(inputs);
        StepIo {
            source,
            sink: Sink::Pipe(Box::new(out)),
            rows: Some(rows),
        }
    }

    /// `step`'s document `part`, checked and deserialised; missing is an error naming its key.
    pub fn get<T>(&self, step: StepName, part: Option<&str>) -> Result<T>
    where
        T: Archive,
        T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>
            + Deserialize<T, HighDeserializer<ArchiveError>>,
    {
        let address = keys::output_address(step, part);
        self.read(&address)?.ok_or_else(|| self.missing(&address))
    }

    /// `f` of `step`'s document `part`, read in place from its checked archive; missing is an
    /// error naming its key.
    pub fn view<T, R>(
        &self,
        step: StepName,
        part: Option<&str>,
        f: impl FnOnce(&T::Archived) -> R,
    ) -> Result<R>
    where
        T: Archive,
        T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>,
    {
        let address = keys::output_address(step, part);
        let shown = shown(&address);
        let viewed = match &self.source {
            Source::Store(read) => read.view::<T, R>(address.table, &address.key, f)?,
            Source::Received(received) => match received.get(&address) {
                None => None,
                Some(bytes) => {
                    let archived = rkyv::access::<T::Archived, ArchiveError>(bytes)
                        .context(format!("the input {shown} does not read"))?;
                    Some(f(archived))
                }
            },
        };
        viewed.ok_or_else(|| self.missing(&address))
    }

    /// The value at `address`, checked and deserialised; `None` when it is not there.
    pub fn read<T>(&self, address: &Address) -> Result<Option<T>>
    where
        T: Archive,
        T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>
            + Deserialize<T, HighDeserializer<ArchiveError>>,
    {
        match &self.source {
            Source::Store(read) => read.get(address.table, &address.key),
            Source::Received(received) => received
                .get(address)
                .map(|bytes| {
                    rkyv::from_bytes::<T, ArchiveError>(bytes)
                        .context(format!("the input {} does not read", shown(address)))
                })
                .transpose(),
        }
    }

    /// The job record.
    pub fn job_record(&self) -> Result<JobRecord> {
        let address = keys::job_record_address();
        self.read(&address)?.ok_or_else(|| self.missing(&address))
    }

    /// The probe-and-decode result.
    pub fn probe(&self) -> Result<ProbeDecoded> {
        self.get(StepName::ProbeDecode, None)
    }

    /// The owner's line corrections; none when the job has none.
    pub fn corrections(&self) -> Result<Corrections> {
        Ok(self
            .read(&keys::corrections_address(keys::LINE_CORRECTIONS))?
            .unwrap_or_default())
    }

    /// The owner's on-screen text corrections; none when the job has none.
    pub fn text_corrections(&self) -> Result<TextCorrections> {
        Ok(self
            .read(&keys::corrections_address(keys::TEXT_CORRECTIONS))?
            .unwrap_or_default())
    }

    /// Keep `value` as `step`'s document `part`.
    pub fn put<T>(&mut self, step: StepName, part: Option<&str>, value: &T) -> Result<()>
    where
        T: for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, ArchiveError>>,
    {
        let address = keys::output_address(step, part);
        kinds::kind(address.table, &address.key)?;
        self.send(&address, value)
    }

    /// Keep `value` as the row of `occurrence`'s frame `frame` in `table` (`frames` or
    /// `readings`).
    pub fn put_frame<T>(
        &mut self,
        table: Table,
        occurrence: &str,
        frame: u64,
        value: &T,
    ) -> Result<()>
    where
        T: for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, ArchiveError>>,
    {
        if !matches!(table, Table::Frames | Table::Readings) {
            return Err(PipelineError::new(
                format!("table {table}"),
                "a per-frame row goes in the frames or readings table",
            ));
        }
        let address = Address {
            table,
            key: Key::Frame {
                occurrence: occurrence.to_string(),
                frame,
            },
        };
        self.send(&address, value)
    }

    /// `f` of every row of the per-frame `table`, in key order, as its occurrence, its frame and
    /// its checked archive read in place; one row is held at a time. In the runner the rows come
    /// from the step's snapshot; in a worker from its stdin, after its documents, once per table.
    pub fn frame_rows<T>(
        &mut self,
        table: Table,
        mut f: impl FnMut(&str, u64, &T::Archived) -> Result<()>,
    ) -> Result<()>
    where
        T: Archive,
        T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>,
    {
        let mut row = |occurrence: &str, frame: u64, bytes: &[u8]| {
            let archived = rkyv::access::<T::Archived, ArchiveError>(bytes).context(format!(
                "the row {table} {occurrence}/{frame} does not read"
            ))?;
            f(occurrence, frame, archived)
        };
        match (&self.source, &mut self.rows) {
            (Source::Store(read), _) => read.rows(table, None, row),
            (Source::Received(_), Some(rows)) => rows.each(table, &mut row),
            (Source::Received(_), None) => Err(PipelineError::new(
                format!("the {table} rows"),
                "the runner sends this step no rows",
            )),
        }
    }

    /// Read whatever the runner sent that the task did not read, to the end of the pipe, so the
    /// runner's sending thread ends cleanly; nothing in the runner.
    pub fn finish_inputs(&mut self) -> Result<()> {
        match &mut self.rows {
            Some(rows) => rows.drain(),
            None => Ok(()),
        }
    }

    /// The step's pending write in the runner, for the runner to commit with the step's record;
    /// `None` in a worker, whose outputs already went up the channel.
    pub fn into_outputs(self) -> Option<StepWrite> {
        match self.sink {
            Sink::Store(write) => Some(*write),
            _ => None,
        }
    }

    fn send<T>(&mut self, address: &Address, value: &T) -> Result<()>
    where
        T: for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, ArchiveError>>,
    {
        let shown = shown(address);
        let archive = rkyv::to_bytes::<ArchiveError>(value)
            .context(format!("the output {shown} does not archive"))?;
        match &mut self.sink {
            Sink::Store(write) => write.put(address, &archive),
            Sink::Channel => {
                if worker_channel::worker::output(address, &archive) {
                    Ok(())
                } else {
                    Err(PipelineError::new(
                        format!("the output {shown}"),
                        "the runner's end of the worker channel is closed",
                    ))
                }
            }
            #[cfg(test)]
            Sink::Pipe(out) => {
                let head = address
                    .encode()
                    .context(format!("the output {shown} has no address"))?;
                worker_channel::frame::write_frame(
                    out,
                    worker_channel::frame::Tag::Output,
                    &[&head, &archive],
                )
                .context(format!("the output {shown} cannot be sent"))
            }
        }
    }

    fn missing(&self, address: &Address) -> PipelineError {
        let why = match self.source {
            Source::Store(_) => "is not in the job database",
            Source::Received(_) => "is not among the inputs the runner sent",
        };
        PipelineError::new(format!("the input {}", shown(address)), why)
    }
}

/// The documents among `inputs` by address, and its per-frame rows, in order, as a stream of the
/// per-frame tables.
fn received(inputs: Vec<Input>) -> (Source, RowStream) {
    let (rows, documents): (Vec<Input>, Vec<Input>) = inputs
        .into_iter()
        .partition(|(address, _)| matches!(address.key, Key::Frame { .. }));
    let mut stream = Vec::new();
    for (address, archive) in &rows {
        // An address that does not encode never reaches a worker; it is left out here too.
        if let Ok(head) = address.encode() {
            let _ = write_frame(&mut stream, Tag::Input, &[&head, archive]);
        }
    }
    let tables = vec![Table::Frames, Table::Readings];
    let source = Source::Received(documents.into_iter().collect());
    let stream = RowStream::new(tables, None, Box::new(std::io::Cursor::new(stream)));
    (source, stream)
}

/// An address as `<table> <key>`.
fn shown(address: &Address) -> String {
    format!("{} {}", address.table, kinds::shown(&address.key))
}

#[cfg(test)]
#[path = "tests/io.rs"]
mod tests;
