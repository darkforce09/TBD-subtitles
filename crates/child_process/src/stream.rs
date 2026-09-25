//! Draining a child's pipes for the whole of its life.
//!
//! A pipe buffer is about 64 KiB. A parent that reads stdout to the end before touching stderr
//! deadlocks the moment the child fills the stderr buffer: the child blocks writing, the parent
//! blocks reading, and neither moves again. Every capture here therefore hands each pipe to a
//! thread that reads it until EOF, started before the parent waits on the child.
//!
//! Decoding is lossy on purpose. A stray non-UTF-8 byte in a diagnostic or a
//! file name must not lose the exit status of the run that printed it.

use std::io::{PipeReader, Read};
use std::process::Child;
use std::thread::JoinHandle;

/// The two threads draining a child's separate stdout and stderr pipes.
pub(crate) struct SeparateDrains {
    stdout: JoinHandle<String>,
    stderr: JoinHandle<String>,
}

impl SeparateDrains {
    /// Take both pipes off `child` and start reading each on its own thread.
    pub(crate) fn start(child: &mut Child) -> SeparateDrains {
        let mut out_pipe = child.stdout.take();
        let mut err_pipe = child.stderr.take();
        SeparateDrains {
            stdout: std::thread::spawn(move || drain(&mut out_pipe)),
            stderr: std::thread::spawn(move || drain(&mut err_pipe)),
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
pub(crate) fn start_merged_drain(mut reader: PipeReader) -> JoinHandle<String> {
    std::thread::spawn(move || read_to_string_lossy(&mut reader))
}

fn drain(pipe: &mut Option<impl Read>) -> String {
    match pipe.as_mut() {
        Some(p) => read_to_string_lossy(p),
        None => String::new(),
    }
}

/// Read to EOF, replacing invalid UTF-8 instead of refusing the bytes.
pub(crate) fn read_to_string_lossy(source: &mut impl Read) -> String {
    let mut buf = Vec::new();
    let _ = source.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}
