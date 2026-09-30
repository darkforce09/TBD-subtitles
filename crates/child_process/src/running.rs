//! A child that runs while the caller reads its stdout as a stream.
//!
//! **Role:** [`Run::spawn`] starts a child whose stdout the caller reads itself (FFmpeg's PCM
//! pipe, a worker's progress) while stderr is drained on its own thread, and hands back a
//! [`Running`] handle with the child's pid, its stdout, a piped stdin the caller streams into
//! (FFmpeg's raw video input), a kill switch and a reaping wait.
//!
//! **Position:** called by `media_io` for FFmpeg streams and by the job runner and the stack
//! spike tool for worker processes; uses `runner.rs` to build and spawn the command and
//! `stream.rs` to drain stderr.
//!
//! **Signals and state:** one watchdog thread per child with a deadline or a cancel flag; it kills
//! the process group when the deadline passes or the flag is set, even while the caller is blocked
//! reading stdout or blocked writing stdin (the write then fails with a broken pipe).
//!
//! **Invariants:** a dropped handle that was never waited on kills its process group, so an
//! abandoned stream never leaves FFmpeg running; a deadline reports [`RunError::Timeout`], a
//! cancel [`RunError::Cancelled`], a signal [`RunError::Signalled`], never an invented exit code.
//! Only the direct child's group is killed; children it started in groups of their own die with
//! it through `PR_SET_PDEATHSIG`.

use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::runner::{feed_stdin, spawn};
use crate::stream::start_stderr_drain;
use crate::trace::Tag;
use crate::{Run, RunError, Stdin};

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
    tag: Tag,
    started: Instant,
    limit: Option<Duration>,
    stderr: Option<JoinHandle<String>>,
    finished: Arc<AtomicBool>,
    timed_out: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    reaped: bool,
}

impl Run {
    /// Start the child with stdout piped to the caller and stderr drained on a thread.
    pub fn spawn(self) -> Result<Running, RunError> {
        let label = self.display();
        let mut cmd = self.command(Stdio::piped(), Stdio::piped());
        let mut child = spawn(&mut cmd, &self.program, &label)?;
        let pgid = child.id() as i32;
        let tag = Tag::started(&self, child.id());
        if self.stdin != Stdin::Piped {
            feed_stdin(&mut child, &self.stdin);
        }
        let stderr = child
            .stderr
            .take()
            .map(|pipe| start_stderr_drain(pipe, &tag));
        let finished = Arc::new(AtomicBool::new(false));
        let timed_out = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));
        if self.timeout.is_some() || self.cancel.is_some() {
            start_watchdog(
                pgid,
                Watch {
                    deadline: self.timeout.map(|limit| Instant::now() + limit),
                    cancel: self.cancel.clone(),
                    finished: finished.clone(),
                    timed_out: timed_out.clone(),
                    cancelled: cancelled.clone(),
                },
            );
        }
        Ok(Running {
            child,
            pgid,
            label,
            tag,
            started: Instant::now(),
            limit: self.timeout,
            stderr,
            finished,
            timed_out,
            cancelled,
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

    /// The child's stdin when the run was [`Run::stdin_piped`]; `None` otherwise and after the
    /// first call. Dropping it closes the pipe, so the child sees EOF.
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
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
    /// full pipe cannot keep the wait from returning; an untaken piped stdin is closed too, so a
    /// child reading it sees EOF. A caller that took the stdin drops it before waiting.
    pub fn wait(mut self) -> Result<Finished, RunError> {
        let finished = self.reap();
        match &finished {
            Ok(done) => self.tag.exited(done.code, done.duration),
            Err(error) => self.tag.failed(error),
        }
        finished
    }

    fn reap(&mut self) -> Result<Finished, RunError> {
        drop(self.child.stdin.take());
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
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(RunError::Cancelled {
                program: self.label.clone(),
            });
        }
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
            self.tag.abandoned();
        }
    }
}

/// What a watchdog watches, and where it records why it killed.
struct Watch {
    deadline: Option<Instant>,
    cancel: Option<Arc<AtomicBool>>,
    finished: Arc<AtomicBool>,
    timed_out: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}

/// Kill the group at the deadline or once the cancel flag is set, unless the child is reaped
/// first.
fn start_watchdog(pgid: i32, watch: Watch) {
    std::thread::spawn(move || {
        while !watch.finished.load(Ordering::SeqCst) {
            if watch
                .cancel
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::SeqCst))
            {
                watch.cancelled.store(true, Ordering::SeqCst);
                kill_group(pgid);
                return;
            }
            if watch.deadline.is_some_and(|d| Instant::now() >= d) {
                watch.timed_out.store(true, Ordering::SeqCst);
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
