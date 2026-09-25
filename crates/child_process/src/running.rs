//! A child that runs while the caller reads its stdout as a stream.
//!
//! **Role:** [`Run::spawn`] starts a child whose stdout the caller reads itself (FFmpeg's PCM
//! pipe, a worker's progress) while stderr is drained on its own thread, and hands back a
//! [`Running`] handle with the child's pid, its stdout, a kill switch and a reaping wait.
//!
//! **Position:** called by `media_io` for FFmpeg streams and by the job runner and the stack
//! spike tool for worker processes; uses `runner.rs` to build and spawn the command and
//! `stream.rs` to drain stderr.
//!
//! **Signals and state:** one watchdog thread per child with a deadline; it kills the process
//! group when the deadline passes, even while the caller is blocked reading stdout.
//!
//! **Invariants:** a dropped handle that was never waited on kills its process group, so an
//! abandoned stream never leaves FFmpeg running; a deadline reports [`RunError::Timeout`], a
//! signal [`RunError::Signalled`], never an invented exit code.

use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ChildStdout, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::runner::{feed_stdin, spawn};
use crate::stream::read_to_string_lossy;
use crate::{Run, RunError};

/// How often the watchdog looks at the clock.
const WATCH: Duration = Duration::from_millis(50);

/// What a streamed child left behind once reaped.
#[derive(Debug)]
pub struct Finished {
    pub code: i32,
    pub stderr: String,
    pub duration: Duration,
}

/// A running child whose stdout belongs to the caller.
pub struct Running {
    child: Child,
    pgid: i32,
    label: String,
    started: Instant,
    limit: Option<Duration>,
    stderr: Option<JoinHandle<String>>,
    finished: Arc<AtomicBool>,
    timed_out: Arc<AtomicBool>,
    reaped: bool,
}

impl Run {
    /// Start the child with stdout piped to the caller and stderr drained on a thread.
    pub fn spawn(self) -> Result<Running, RunError> {
        let label = self.display();
        let mut cmd = self.command(Stdio::piped(), Stdio::piped());
        let mut child = spawn(&mut cmd, &self.program, &label)?;
        let pgid = child.id() as i32;
        feed_stdin(&mut child, self.stdin.as_deref());
        let stderr = child
            .stderr
            .take()
            .map(|mut pipe| std::thread::spawn(move || read_to_string_lossy(&mut pipe)));
        let finished = Arc::new(AtomicBool::new(false));
        let timed_out = Arc::new(AtomicBool::new(false));
        if let Some(limit) = self.timeout {
            start_watchdog(pgid, limit, finished.clone(), timed_out.clone());
        }
        Ok(Running {
            child,
            pgid,
            label,
            started: Instant::now(),
            limit: self.timeout,
            stderr,
            finished,
            timed_out,
            reaped: false,
        })
    }
}

impl Running {
    /// The child's process id, which is also its process-group id.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// The child's stdout; `None` after the first call.
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    /// Kill the child's whole process group now.
    pub fn kill(&mut self) {
        kill_group(self.pgid);
    }

    /// Whether the child has exited, without blocking.
    pub fn has_exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    /// Reap the child and collect its stderr.
    ///
    /// Stdout is closed first if the caller still holds it unread, so a child blocked writing to a
    /// full pipe cannot keep the wait from returning.
    pub fn wait(mut self) -> Result<Finished, RunError> {
        drop(self.child.stdout.take());
        let status = self.child.wait().map_err(|e| RunError::Failed {
            program: self.label.clone(),
            message: format!("wait failed: {e}"),
        });
        self.reaped = true;
        self.finished.store(true, Ordering::SeqCst);
        let stderr = self
            .stderr
            .take()
            .and_then(|h| h.join().ok())
            .unwrap_or_default();
        let status = status?;
        if self.timed_out.load(Ordering::SeqCst) {
            return Err(RunError::Timeout {
                program: self.label.clone(),
                secs: self.limit.map(|l| l.as_secs()).unwrap_or(0),
            });
        }
        if let Some(signal) = status.signal() {
            return Err(RunError::Signalled {
                program: self.label.clone(),
                signal,
            });
        }
        Ok(Finished {
            code: status.code().unwrap_or(-1),
            stderr,
            duration: self.started.elapsed(),
        })
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if !self.reaped {
            kill_group(self.pgid);
            let _ = self.child.wait();
            self.finished.store(true, Ordering::SeqCst);
        }
    }
}

/// Kill the group at `limit` unless the child is reaped first.
fn start_watchdog(
    pgid: i32,
    limit: Duration,
    finished: Arc<AtomicBool>,
    timed_out: Arc<AtomicBool>,
) {
    let deadline = Instant::now() + limit;
    std::thread::spawn(move || {
        while !finished.load(Ordering::SeqCst) {
            if Instant::now() >= deadline {
                timed_out.store(true, Ordering::SeqCst);
                kill_group(pgid);
                return;
            }
            std::thread::sleep(WATCH);
        }
    });
}

fn kill_group(pgid: i32) {
    // SAFETY: `killpg` on a group this crate created with `setsid`; failure means the group is
    // already gone, which is the wanted outcome.
    unsafe {
        libc::killpg(pgid, libc::SIGKILL);
    }
}

#[cfg(test)]
#[path = "tests/running.rs"]
mod tests;
