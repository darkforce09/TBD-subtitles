//! A `hold` child process of this executable, seen from the matrix.
//!
//! **Role:** starts `redb-process-probe hold` with piped stdin and stdout, waits for its first
//! line, tells whether it is still running, and ends it by closing its stdin or by SIGKILL.
//!
//! **Position:** used by the matrix scenarios; depends on `open_mode` for the argument names.
//!
//! **Signals and state:** a reader thread forwards the child's stdout lines over a channel; the
//! child's stderr is inherited, so a holder's own complaint shows in the terminal.
//!
//! **Invariants:** the child is this same executable (`std::env::current_exe`), never another
//! program; every started child is waited for, on every path: `close`, `kill` and `exited` reap
//! it, their error paths still kill and wait, and `Drop` kills and reaps a child nothing reaped,
//! so none outlives the matrix even when a scenario returns early or panics.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Context;

use crate::open_mode::{OpenMode, Sharing};

/// How long a holder may take to print its first line.
pub(crate) const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a holder may take to exit once its stdin is closed, before it is killed.
pub(crate) const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

/// What a holder's first line said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Readiness {
    /// It printed `ready`: the database is open in it.
    Ready,
    /// It printed something else, such as `failed: …`, or nothing in time.
    NotReady(String),
}

/// One running `hold` child; dropping it kills and reaps the child if nothing reaped it yet.
pub(crate) struct HolderProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    reaped: bool,
}

impl HolderProcess {
    /// Starts `hold --db <path> --mode <mode> --sharing <sharing> [--writing]`.
    pub(crate) fn spawn(
        path: &Path,
        mode: OpenMode,
        sharing: Sharing,
        writing: bool,
    ) -> anyhow::Result<HolderProcess> {
        let executable = std::env::current_exe().context("locate this executable")?;
        let mut command = Command::new(&executable);
        command
            .arg("hold")
            .arg("--db")
            .arg(path)
            .args(["--mode", mode.name(), "--sharing", sharing.name()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if writing {
            command.arg("--writing");
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("start {} hold", executable.display()))?;
        let stdin = child.stdin.take();
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("the holder has no stdout");
        };
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(HolderProcess {
            child,
            stdin,
            lines,
            reaped: false,
        })
    }

    /// Waits up to [`READY_TIMEOUT`] for the first line.
    pub(crate) fn wait_ready(&self) -> Readiness {
        match self.lines.recv_timeout(READY_TIMEOUT) {
            Ok(line) if line == "ready" => Readiness::Ready,
            Ok(line) => Readiness::NotReady(line),
            Err(RecvTimeoutError::Timeout) => {
                Readiness::NotReady(format!("no line within {} s", READY_TIMEOUT.as_secs()))
            }
            Err(RecvTimeoutError::Disconnected) => {
                Readiness::NotReady("exited without printing a line".to_string())
            }
        }
    }

    /// `None` while the holder runs; otherwise how it ended, with its last `failed: …` line.
    pub(crate) fn exited(&mut self) -> Option<String> {
        match self.child.try_wait() {
            Ok(None) => None,
            Ok(Some(status)) => {
                self.reaped = true;
                let failure = self.failure_line();
                Some(with_failure(describe_exit(status), failure))
            }
            Err(error) => {
                let ended = self.reap();
                Some(format!("try_wait failed: {error}; {ended}"))
            }
        }
    }

    /// Sends SIGKILL, then closes stdin, then waits, so the holder cannot see end of file and
    /// close cleanly first. `Ok` when SIGKILL ended it; `Err` with how it ended otherwise.
    pub(crate) fn kill(mut self) -> Result<String, String> {
        let killed = self.child.kill();
        self.stdin = None;
        let waited = self.child.wait();
        self.reaped = true;
        match (killed, waited) {
            (Ok(()), Ok(status)) if killed_by_sigkill(status) => {
                Ok(format!("killed: {}", describe_exit(status)))
            }
            (Ok(()), Ok(status)) => Err(format!(
                "not ended by SIGKILL: {}",
                with_failure(describe_exit(status), self.failure_line())
            )),
            (Err(error), Ok(status)) => Err(format!(
                "kill failed: {error}; {}",
                with_failure(describe_exit(status), self.failure_line())
            )),
            (_, Err(error)) => Err(format!("wait failed: {error}")),
        }
    }

    /// Closes stdin and waits up to [`CLOSE_TIMEOUT`] for the exit, killing the child after it.
    pub(crate) fn close(mut self) -> String {
        self.stdin = None;
        let started = Instant::now();
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    self.reaped = true;
                    return format!(
                        "{} {:.0} ms after stdin closed",
                        describe_exit(status),
                        started.elapsed().as_secs_f64() * 1000.0
                    );
                }
                Ok(None) if started.elapsed() < CLOSE_TIMEOUT => {
                    thread::sleep(Duration::from_millis(5));
                }
                Ok(None) => {
                    let ended = self.reap();
                    return format!(
                        "still running {} s after stdin closed; {ended}",
                        CLOSE_TIMEOUT.as_secs()
                    );
                }
                Err(error) => {
                    let ended = self.reap();
                    return format!("try_wait failed: {error}; {ended}");
                }
            }
        }
    }

    /// Kills and waits for the child, whatever the kill returned; says how it ended.
    fn reap(&mut self) -> String {
        let _ = self.child.kill();
        self.stdin = None;
        let waited = self.child.wait();
        self.reaped = true;
        match waited {
            Ok(status) => format!("killed and reaped: {}", describe_exit(status)),
            Err(error) => format!("wait failed: {error}"),
        }
    }

    /// The last `failed: …` line the holder printed after `ready`, once its stdout has ended.
    fn failure_line(&self) -> Option<String> {
        let mut failure = None;
        while let Ok(line) = self.lines.recv_timeout(STDOUT_DRAIN) {
            if line.starts_with("failed: ") {
                failure = Some(line);
            }
        }
        failure
    }
}

impl Drop for HolderProcess {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.reap();
        }
    }
}

/// How long a holder that has exited may take to hand over the rest of its stdout.
const STDOUT_DRAIN: Duration = Duration::from_millis(200);

/// `ended` followed by the holder's own failure line, when it printed one.
fn with_failure(ended: String, failure: Option<String>) -> String {
    match failure {
        Some(line) => format!("{ended} ({line})"),
        None => ended,
    }
}

/// Whether `status` is the end SIGKILL gives a process.
fn killed_by_sigkill(status: ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt;
    status.signal() == Some(SIGKILL)
}

/// The signal number of SIGKILL on Linux.
const SIGKILL: i32 = 9;

/// `exited <code>` or `ended by signal <n>`.
fn describe_exit(status: ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt;
    match (status.code(), status.signal()) {
        (Some(code), _) => format!("exited {code}"),
        (None, Some(signal)) => format!("ended by signal {signal}"),
        (None, None) => format!("ended ({status})"),
    }
}

#[cfg(test)]
#[path = "tests/holder_process.rs"]
mod tests;
