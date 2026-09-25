//! Finding a program before running it, and waiting for a condition without pretending.
//!
//! The three helpers here share one rule with the rest of the crate: an answer that was never
//! obtained is reported as [`RunError`], never as a quiet success. A `PATH` miss is
//! [`RunError::ProgramAbsent`], an exhausted wait is [`RunError::Timeout`], and neither can be mistaken
//! for "the condition held".

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::RunError;

/// Resolve a program on `PATH`, or report it absent.
///
/// Checking up front, rather than discovering mid-run that the program was never there.
pub fn which(program: &str) -> Result<PathBuf, RunError> {
    let path =
        std::env::var_os("PATH").ok_or_else(|| RunError::ProgramAbsent(program.to_string()))?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(program);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(RunError::ProgramAbsent(program.to_string()))
}

/// Retry a fallible operation with a fixed backoff.
///
/// A [`RunError::ProgramAbsent`] is **never** retried: an absent program will still be absent in two
/// seconds, and retrying it only delays the real diagnosis.
pub fn retry<T>(
    attempts: u32,
    backoff: Duration,
    mut f: impl FnMut() -> Result<T, RunError>,
) -> Result<T, RunError> {
    let mut last = None;
    for i in 0..attempts.max(1) {
        match f() {
            Ok(v) => return Ok(v),
            Err(e @ RunError::ProgramAbsent(_)) => return Err(e),
            Err(e) => {
                last = Some(e);
                if i + 1 < attempts {
                    std::thread::sleep(backoff);
                }
            }
        }
    }
    Err(last.unwrap_or_else(|| RunError::ProgramAbsent("retry: no attempt ran".into())))
}

/// Poll until `cond` holds or the deadline passes.
///
/// For the "the socket is up" / "the port is listening" waits whose loop form ends the same way
/// whether the condition held or the attempts ran out; here exhaustion is a [`RunError::Timeout`]
/// and only a true `cond` returns `Ok`.
pub fn wait_for(
    label: &str,
    timeout: Duration,
    poll: Duration,
    mut cond: impl FnMut() -> bool,
) -> Result<(), RunError> {
    let deadline = Instant::now() + timeout;
    loop {
        if cond() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(RunError::Timeout {
                program: label.to_string(),
                secs: timeout.as_secs(),
            });
        }
        std::thread::sleep(poll);
    }
}
