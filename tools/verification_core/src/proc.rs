//! Child processes for checks, with every stop reason kept as a verdict.
//!
//! **Role:** runs the programs a check needs (`git`, `cargo`) through `child_process` and turns
//! each reason a run produced no exit code into the [`NotRun`] cause of a check that did not run.
//!
//! **Position:** used by the gates in `tools/repo_gates`; calls `crates/child_process`, which
//! spawns, isolates, times out and drains every child.
//!
//! **Signals and state:** spawns processes; holds no state.
//!
//! **Invariants:** a missing program, a signal and a timeout each become `DidNotRun`, never a
//! `Failed` and never a pass; exit codes pass through raw.

use std::path::PathBuf;

pub use child_process::{Merged, Output, Run, RunError};

use crate::verdict::{Finding, Kind, NotRun, Verdict};

impl From<RunError> for NotRun {
    fn from(error: RunError) -> NotRun {
        match error {
            RunError::ProgramAbsent(program) => NotRun::ToolAbsent(program),
            RunError::Failed { program, message } => NotRun::ToolError {
                tool: program,
                status: -1,
                stderr: message,
            },
            RunError::Signalled { program, signal } => NotRun::Signalled {
                tool: program,
                signal,
            },
            RunError::Timeout { program, secs } => NotRun::Timeout {
                tool: program,
                secs,
            },
        }
    }
}

/// Verdicts over a whole run: the check holds on one exact exit code.
pub trait RunVerdict {
    /// A [`Verdict`] that holds when the command exits 0.
    fn expect_ok(self, msg: &str) -> Verdict;
    /// A [`Verdict`] that holds only on exactly `want`, for a command whose success is non-zero.
    fn expect_code(self, msg: &str, want: i32) -> Verdict;
}

impl RunVerdict for Run {
    fn expect_ok(self, msg: &str) -> Verdict {
        self.expect_code(msg, 0)
    }

    fn expect_code(self, msg: &str, want: i32) -> Verdict {
        let label = self.display();
        match self.output() {
            Err(cause) => Verdict::did_not_run(msg, Kind::Pin, cause.into()),
            Ok(out) if out.code == want => Verdict::Held,
            Ok(out) => Verdict::Failed(Finding {
                headline: format!("{msg} — `{label}` exited {} (want {want})", out.code),
                detail: out.stderr.lines().take(10).map(str::to_string).collect(),
            }),
        }
    }
}

/// Resolve a program on `PATH`, or report the check unable to run.
pub fn which(program: &str) -> Result<PathBuf, NotRun> {
    child_process::which(program).map_err(NotRun::from)
}

#[cfg(test)]
#[path = "tests/proc_tests.rs"]
mod tests;
