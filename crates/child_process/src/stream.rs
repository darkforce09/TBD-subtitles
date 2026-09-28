//! Draining a child's pipes for the whole of its life.
//!
//! **Role:** read each pipe of a child on a thread of its own until EOF, logging stderr (and a
//! shared pipe) line by line as it arrives, and hand back every byte as text.
//!
//! **Position:** used by `runner.rs` for `output` and `merged_output` and by `running.rs` for a
//! streamed child's stderr; logs each line through `trace.rs`.
//!
//! **Signals and state:** one thread per pipe; no state once the pipe ends.
//!
//! **Invariants:** a full pipe never deadlocks a child. A pipe buffer is about 64 KiB, and a parent
//! that reads stdout to the end before touching stderr deadlocks the moment the child fills the
//! stderr buffer, so every drain starts before the parent waits on the child. Decoding is lossy
//! on purpose: a stray non-UTF-8 byte in a diagnostic or a file name must not lose the exit status
//! of the run that printed it. The text handed back is every byte read, logged or not.

use std::io::{BufRead, BufReader, PipeReader, Read};
use std::process::Child;
use std::thread::JoinHandle;

use crate::trace::Tag;

/// The two threads draining a child's separate stdout and stderr pipes.
pub(crate) struct SeparateDrains {
    stdout: JoinHandle<String>,
    stderr: JoinHandle<String>,
}

impl SeparateDrains {
    /// Take both pipes off `child` and start reading each on its own thread; stderr lines are
    /// logged under `tag`.
    pub(crate) fn start(child: &mut Child, tag: &Tag) -> SeparateDrains {
        let mut out_pipe = child.stdout.take();
        let err_pipe = child.stderr.take();
        let tag = tag.clone();
        SeparateDrains {
            stdout: std::thread::spawn(move || match out_pipe.as_mut() {
                Some(pipe) => read_to_string_lossy(pipe),
                None => String::new(),
            }),
            stderr: std::thread::spawn(move || match err_pipe {
                Some(pipe) => read_logging_lines(pipe, &tag),
                None => String::new(),
            }),
        }
    }

    /// Wait for both threads and hand back `(stdout, stderr)`.
    ///
    /// A panicked reader yields an empty string rather than propagating: the child's status is
    /// the verdict, and losing captured text must not turn into losing the exit code.
    pub(crate) fn join(self) -> (String, String) {
        let stdout = self.stdout.join().unwrap_or_default();
        let stderr = self.stderr.join().unwrap_or_default();
        (stdout, stderr)
    }
}

/// Start the single thread reading a shared pipe that carries stdout and stderr interleaved.
pub(crate) fn start_merged_drain(reader: PipeReader, tag: &Tag) -> JoinHandle<String> {
    let tag = tag.clone();
    std::thread::spawn(move || read_logging_lines(reader, &tag))
}

/// Start the thread reading a streamed child's stderr.
pub(crate) fn start_stderr_drain(
    pipe: impl Read + Send + 'static,
    tag: &Tag,
) -> JoinHandle<String> {
    let tag = tag.clone();
    std::thread::spawn(move || read_logging_lines(pipe, &tag))
}

/// Read to EOF, logging each line under `tag`, and hand back every byte read.
fn read_logging_lines(source: impl Read, tag: &Tag) -> String {
    let mut reader = BufReader::new(source);
    let mut all = Vec::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                tag.line(&String::from_utf8_lossy(&line));
                all.extend_from_slice(&line);
            }
        }
    }
    String::from_utf8_lossy(&all).into_owned()
}

/// Read to EOF, replacing invalid UTF-8 instead of refusing the bytes.
fn read_to_string_lossy(source: &mut impl Read) -> String {
    let mut buf = Vec::new();
    let _ = source.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}
