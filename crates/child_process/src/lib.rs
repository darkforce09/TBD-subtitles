//! Running external programs without losing the reason they stopped.
//!
//! **Role:** the one way the app and the repository tools start a child process: FFmpeg, ffprobe,
//! the `claude` CLI, the app's own GPU workers, and `git` or `cargo` in the tools.
//!
//! **Position:** the bottom layer, beside `job_model`; `media_io`, `inference`, `pipeline` and
//! `tools/verification_core` call it. It calls only `std`, `libc` and the `tracing` facade.
//!
//! **Signals and state:** reads the `PATH` environment variable in [`which`]; spawns processes;
//! holds no state between runs.
//!
//! **Invariants:**
//!
//! 1. A signal is not an exit code. A child killed by SIGKILL has no exit code; synthesising
//!    `128+n` would hand the caller a 137 that reads as an ordinary failure. Here it is
//!    [`RunError::Signalled`].
//! 2. A deadline kills the tree, not only the direct child. FFmpeg, `cargo` and the GPU workers
//!    may fork; every child runs in its own process group via `setsid`, and a timeout kills the
//!    group.
//! 3. A full pipe never deadlocks a captured child. Both streams are drained by dedicated threads
//!    for the child's whole life.
//! 4. A child never outlives the thread that started it: the kernel kills it (`PR_SET_PDEATHSIG`)
//!    when that thread ends, so a killed app leaves no worker holding GPU memory. Start a child
//!    only from a thread that stays alive until the child is reaped.
//!
//! Exit codes pass through raw: [`Run::status`] hands back the real code, because a caller may
//! need an exact non-zero code. `runner.rs` spawns, isolates and reaps; `running.rs` hands a
//! streamed stdout to the caller under a watchdog deadline; `stream.rs` drains the pipes;
//! `trace.rs` logs each child's start, stderr lines and end as `tracing` events; `lookup.rs`
//! resolves programs on `PATH` and waits on conditions.

mod lookup;
mod runner;
mod running;
mod stream;
mod trace;

pub use lookup::{retry, wait_for, which};
pub use running::{Finished, Running};

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// What a finished process produced.
#[derive(Debug)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
}

/// What a finished process produced on one shared pipe; see [`Run::merged_output`].
///
/// A separate type rather than an [`Output`] with an always-empty `stderr`: the two streams are no
/// longer separable, and an always-empty field invites the conclusion that the child wrote
/// nothing to stderr.
#[derive(Debug)]
pub struct Merged {
    pub code: i32,
    /// stdout and stderr interleaved exactly as the child emitted them.
    pub text: String,
    pub duration: Duration,
}

/// Why a run produced no exit code.
///
/// Each variant means the program never reported a result, and each sends the reader somewhere
/// different: the machine (`ProgramAbsent`), the operating system (`Failed`), the kernel or a
/// user (`Signalled`), the deadline (`Timeout`), or the caller's cancel flag (`Cancelled`).
#[derive(Debug)]
pub enum RunError {
    /// The program is on no `PATH` entry: the honest form of exit 127.
    ProgramAbsent(String),
    /// Spawning, waiting or piping failed before the program could report.
    Failed { program: String, message: String },
    /// The child died on a signal.
    Signalled { program: String, signal: i32 },
    /// The run exceeded its deadline and its process group was killed.
    Timeout { program: String, secs: u64 },
    /// The caller's cancel flag was set and the process group was killed.
    Cancelled { program: String },
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::ProgramAbsent(program) => write!(f, "{program} not found on PATH"),
            RunError::Failed { program, message } => write!(f, "{program}: {message}"),
            RunError::Signalled { program, signal } => {
                write!(f, "{program} killed by signal {signal}")
            }
            RunError::Timeout { program, secs } => write!(f, "{program} timed out after {secs}s"),
            RunError::Cancelled { program } => write!(f, "{program} was cancelled"),
        }
    }
}

impl std::error::Error for RunError {}

/// A command to run.
pub struct Run {
    program: String,
    args: Vec<String>,
    cwd: Option<PathBuf>,
    envs: Vec<(String, String)>,
    env_removes: Vec<String>,
    timeout: Option<Duration>,
    cancel: Option<Arc<AtomicBool>>,
    stdin: Option<String>,
}

impl Run {
    pub fn new(program: impl AsRef<OsStr>) -> Run {
        Run {
            program: program.as_ref().to_string_lossy().into_owned(),
            args: Vec::new(),
            cwd: None,
            envs: Vec::new(),
            env_removes: Vec::new(),
            timeout: None,
            cancel: None,
            stdin: None,
        }
    }

    pub fn arg(mut self, a: impl AsRef<OsStr>) -> Run {
        self.args.push(a.as_ref().to_string_lossy().into_owned());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Run
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for a in args {
            self.args.push(a.as_ref().to_string_lossy().into_owned());
        }
        self
    }

    pub fn cwd(mut self, dir: impl AsRef<Path>) -> Run {
        self.cwd = Some(dir.as_ref().to_path_buf());
        self
    }

    pub fn env(mut self, k: impl Into<String>, v: impl Into<String>) -> Run {
        self.envs.push((k.into(), v.into()));
        self
    }

    pub fn env_remove(mut self, k: impl Into<String>) -> Run {
        self.env_removes.push(k.into());
        self
    }

    pub fn timeout(mut self, d: Duration) -> Run {
        self.timeout = Some(d);
        self
    }

    /// Kill the child's process group once `flag` is set. Honoured by [`Run::spawn`], whose
    /// watchdog watches the flag; the collecting runs (`output`, `status`) ignore it.
    pub fn cancel_on(mut self, flag: Arc<AtomicBool>) -> Run {
        self.cancel = Some(flag);
        self
    }

    pub fn stdin(mut self, body: impl Into<String>) -> Run {
        self.stdin = Some(body.into());
        self
    }

    /// A human-readable rendering of the command, for diagnostics.
    pub fn display(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

#[cfg(test)]
#[path = "tests/runner.rs"]
mod tests;
