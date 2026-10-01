//! The worker's side of the channel: a private descriptor for frames, and one call per frame
//! kind.
//!
//! **Role:** [`install`] keeps a private copy of the worker's stdout pipe for frames and points
//! descriptor 1 at stderr; [`progress`], [`model_call`], [`output`], [`measure`], [`failed`] and
//! [`done`] send one frame each; [`read_inputs`] reads the `Input` frames the runner writes to the
//! worker's stdin, and [`read_documents`] reads the named ones up to the per-frame rows that
//! follow them, which the step then reads one at a time with [`read_input`].
//!
//! **Position:** called by `pipeline::tasks::worker_main` first thing in a worker, before any
//! native library loads, and by the model-call logging layers of the app binaries.
//!
//! **Signals and state:** one process-wide [`FrameSink`] over the private descriptor, set once by
//! [`install`]; descriptor 1 changes once, to a copy of descriptor 2.
//!
//! **Invariants:** the private descriptor is close-on-exec, so FFmpeg and `claude` never inherit
//! the frame pipe; anything that writes to descriptor 1 after [`install`] (whisper.cpp, ONNX
//! Runtime, mistral.rs, `println!`) lands in the step log, never in the frame stream; a send never
//! panics and answers `false` when the channel is not installed or the pipe is gone; one lock
//! keeps frames from different threads whole.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::AsFd;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::address::{Address, Key};
use crate::frame::{self, Tag};
use crate::progress::Progress;

/// The process's frame channel, once installed.
static FRAMES: OnceLock<FrameSink<File>> = OnceLock::new();

/// Frames written whole to one stream, whatever thread sends them.
pub(crate) struct FrameSink<W: Write> {
    out: Mutex<W>,
}

impl<W: Write> FrameSink<W> {
    pub(crate) fn new(out: W) -> FrameSink<W> {
        FrameSink {
            out: Mutex::new(out),
        }
    }

    /// Write one frame under the lock.
    pub(crate) fn send(&self, tag: Tag, parts: &[&[u8]]) -> io::Result<()> {
        frame::write_frame(&mut *lock(&self.out), tag, parts)
    }

    pub(crate) fn progress(&self, done: u64, total: u64) -> io::Result<()> {
        self.send(Tag::Progress, &[&Progress { done, total }.encode()])
    }

    pub(crate) fn model_call(&self, json: &str) -> io::Result<()> {
        self.send(Tag::ModelCall, &[json.as_bytes()])
    }

    pub(crate) fn output(&self, address: &Address, archive: &[u8]) -> io::Result<()> {
        self.send(Tag::Output, &[&address.encode()?, archive])
    }

    pub(crate) fn measure(&self, archive: &[u8]) -> io::Result<()> {
        self.send(Tag::Measure, &[archive])
    }

    pub(crate) fn failed(&self, message: &str) -> io::Result<()> {
        self.send(Tag::Failed, &[message.as_bytes()])
    }

    pub(crate) fn done(&self) -> io::Result<()> {
        self.send(Tag::Done, &[])
    }
}

/// Keep a close-on-exec copy of stdout for frames and point descriptor 1 at stderr. Call it
/// first thing in a worker; a second call does nothing.
pub fn install() -> io::Result<()> {
    static INSTALLING: Mutex<()> = Mutex::new(());
    let _installing = lock(&INSTALLING);
    if FRAMES.get().is_some() {
        return Ok(());
    }
    let stdout = io::stdout();
    let mut held = stdout.lock();
    held.flush()?;
    let frames = held.as_fd().try_clone_to_owned()?;
    rustix::stdio::dup2_stdout(io::stderr().as_fd())?;
    drop(held);
    let _ = FRAMES.set(FrameSink::new(File::from(frames)));
    Ok(())
}

/// Send one frame; `false` when the channel is not installed or the write failed.
pub fn send(tag: Tag, parts: &[&[u8]]) -> bool {
    with_sink(|sink| sink.send(tag, parts))
}

/// Report that `done` of the step's `total` units are finished.
pub fn progress(done: u64, total: u64) -> bool {
    with_sink(|sink| sink.progress(done, total))
}

/// Send one language-model call, as the JSON of its record.
pub fn model_call(json: &str) -> bool {
    with_sink(|sink| sink.model_call(json))
}

/// Send one value the step writes: its address and its `rkyv` archive.
pub fn output(address: &Address, archive: &[u8]) -> bool {
    with_sink(|sink| sink.output(address, archive))
}

/// Send the worker's measure of itself, as its `rkyv` archive.
pub fn measure(archive: &[u8]) -> bool {
    with_sink(|sink| sink.measure(archive))
}

/// Report why the step failed.
pub fn failed(message: &str) -> bool {
    with_sink(|sink| sink.failed(message))
}

/// Report that the step finished.
pub fn done() -> bool {
    with_sink(|sink| sink.done())
}

/// One value the runner sent: its address and its `rkyv` archive.
pub type Input = (Address, Vec<u8>);

/// Read the next `Input` frame; `None` when the stream ends cleanly.
pub fn read_input(r: &mut impl Read) -> io::Result<Option<Input>> {
    let Some(frame) = frame::read_frame(r)? else {
        return Ok(None);
    };
    if frame.tag != Tag::Input {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("a {} frame among the step's inputs", frame.tag.name()),
        ));
    }
    let (address, taken) = Address::read(&mut frame.payload.as_slice())?;
    let mut payload = frame.payload;
    let archive = payload.split_off(taken);
    Ok(Some((address, archive)))
}

/// Read `Input` frames until the stream ends cleanly: each value's address and its archive.
pub fn read_inputs(r: &mut impl Read) -> io::Result<Vec<Input>> {
    let mut inputs = Vec::new();
    while let Some(input) = read_input(r)? {
        inputs.push(input);
    }
    Ok(inputs)
}

/// Read the named `Input` frames the runner sends first, up to the first per-frame row or the
/// end of the stream; that row, when there is one, comes back beside them, and the rows after it
/// stay on `r` for the step to read one at a time.
pub fn read_documents(r: &mut impl Read) -> io::Result<(Vec<Input>, Option<Input>)> {
    let mut documents = Vec::new();
    while let Some(input) = read_input(r)? {
        if matches!(input.0.key, Key::Frame { .. }) {
            return Ok((documents, Some(input)));
        }
        documents.push(input);
    }
    Ok((documents, None))
}

fn with_sink(send: impl FnOnce(&FrameSink<File>) -> io::Result<()>) -> bool {
    FRAMES.get().is_some_and(|sink| send(sink).is_ok())
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
#[path = "tests/worker.rs"]
mod tests;
