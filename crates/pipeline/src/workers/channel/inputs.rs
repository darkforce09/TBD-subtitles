//! A step's inputs, sent from the job database down its worker's stdin.
//!
//! **Role:** `send_inputs` reads every value a step reads in one read snapshot and writes each as
//! an `Input` frame (its address, then its archive) on a thread of its own, then every row of the
//! per-frame tables the step reads, one `Input` frame per row in key order, then closes the pipe.
//!
//! **Position:** started by `workers::run_worker` for a step that reads stored values, and joined
//! after the worker's frames are read; the worker reads the frames with
//! `worker_channel::worker::read_documents`, then the rows one at a time.
//!
//! **Signals and state:** one thread per step with inputs, holding one read snapshot and the
//! worker's stdin until every frame is written or a write fails; one row at a time, read in place
//! from its page, so a per-frame table is never held whole; the pipe's capacity holds the writer
//! back until the worker reads on.
//!
//! **Invariants:** the frames go out on their own thread, so a worker that reports progress before
//! it reads every input never deadlocks its runner; each archive is written from the database's
//! page in place; a value missing from the database is an error, never an empty frame; the rows
//! come after every named value, so a worker reads its documents before the first row; the pipe
//! is closed when the thread ends, however it ends.

use std::io::Write;
use std::sync::Arc;
use std::thread::JoinHandle;

use worker_channel::address::{Address, Key, Table};
use worker_channel::frame::{self, Tag};

use crate::error::PipelineError;
use crate::work_dir::JobStore;
use crate::work_dir::store::kinds;

/// Write the value at each of `inputs` from `store` to `stdin` as `Input` frames, in order, then
/// every row of each of `rows`, on a thread of its own, then close `stdin`. The thread answers why
/// it stopped early, if it did.
pub(crate) fn send_inputs<W>(
    store: Arc<JobStore>,
    inputs: Vec<Address>,
    rows: &'static [Table],
    stdin: W,
) -> JoinHandle<Result<(), String>>
where
    W: Write + Send + 'static,
{
    std::thread::spawn(move || {
        let mut stdin = stdin;
        let read = store
            .read()
            .map_err(|error| format!("the step's inputs cannot be read: {error}"))?;
        for address in &inputs {
            let shown = format!("{} {}", address.table, kinds::shown(&address.key));
            let head = address
                .encode()
                .map_err(|error| format!("the input {shown} has no address: {error}"))?;
            let sent = read
                .with_bytes(address.table, &address.key, |archive| {
                    Ok(frame::write_frame(
                        &mut stdin,
                        Tag::Input,
                        &[&head, archive],
                    ))
                })
                .map_err(|error| format!("the input {shown} cannot be read: {error}"))?;
            match sent {
                None => return Err(format!("the input {shown} is not in the job database")),
                Some(Err(error)) => {
                    return Err(format!("the input {shown} could not be sent: {error}"));
                }
                Some(Ok(())) => {}
            }
        }
        for table in rows {
            read.rows(*table, None, |occurrence, frame, archive| {
                let address = Address {
                    table: *table,
                    key: Key::Frame {
                        occurrence: occurrence.to_string(),
                        frame,
                    },
                };
                let shown = format!("{table} {occurrence}/{frame}");
                let head = address
                    .encode()
                    .map_err(|error| PipelineError::new(&shown, error.to_string()))?;
                frame::write_frame(&mut stdin, Tag::Input, &[&head, archive])
                    .map_err(|error| PipelineError::new(&shown, format!("cannot be sent: {error}")))
            })
            .map_err(|error| format!("the {table} rows could not be sent: {error}"))?;
        }
        Ok(())
    })
}
