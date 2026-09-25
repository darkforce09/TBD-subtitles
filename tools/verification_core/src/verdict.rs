//! The three-outcome verdict — the whole reason this crate exists.
//!
//! **Role:** [`Verdict`] with its three outcomes, [`NotRun`] naming why a check did not run, and
//! [`Finding`], the rendered failure.
//!
//! **Position:** the vocabulary every other module and every gate speaks.
//!
//! **Signals and state:** none; plain data and its rendering.
//!
//! **Invariants:** a verdict has no conversion to or from `bool`; a failure renders as one headline
//! and six-space continuation lines.
//!
//! ── WHY A TYPE AND NOT A BOOL ────────────────────────────────────────────────────────────────
//!
//! A boolean cannot carry three outcomes. A check reduced to "did the search match?" collapses
//! them into two:
//!
//!   match found          -> ban violated       -> correctly FAILED
//!   no match             -> ban holds          -> correctly passed
//!   TARGET FILE MISSING  -> check never ran    -> reads as passed
//!   SEARCH TOOL ABSENT   -> check never ran    -> reads as passed
//!
//! The last two are this program's signature defect — *a tool reporting success over an input it
//! never examined*. A renamed directory or an uninstalled binary then keeps a gate green for as
//! long as nobody re-derives what it is actually reading.
//!
//! Keeping them apart cannot be left to discipline, because discipline does not propagate to
//! the gate somebody writes next month. So [`Verdict`] has no `From<bool>`, no
//! `Deref<Target = bool>`, no `is_ok()`, and no `PartialEq<bool>`: there is no expression that
//! turns it into a two-way branch by accident. A caller must `match`, and `match` is exhaustive,
//! so **"the check did not run" cannot be silently folded into "the check passed" — that is a
//! compile error rather than a code review.** Adding a variant to [`NotRun`] breaks every
//! incomplete `match` in the workspace, which is what makes the rule spread on its own.
//!
//! ── OUTPUT IS A CONTRACT ─────────────────────────────────────────────────────────────────────
//!
//! [`Finding`] renders one headline plus six-space-indented continuation lines, the same in every
//! gate, so a reader tells a continuation line from the next headline at a glance.

use std::fmt;
use std::path::PathBuf;

/// Which side of the check failed, for the one line of prose that differs between them.
///
/// An enum rather than a `"ban"`/`"pin"` string, for the usual reason: there is no third
/// spelling, and a typo in one must not print a sentence with a hole in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The pattern must NOT appear.
    Ban,
    /// The pattern MUST appear.
    Pin,
}

impl Kind {
    /// The noun interpolated into "The {what} could not run."
    pub fn noun(self) -> &'static str {
        match self {
            Kind::Ban => "ban",
            Kind::Pin => "pin",
        }
    }
}

/// Why a check could not reach a verdict.
///
/// Every variant means the same thing to a caller — *the input was never examined* — but they
/// send a reader to three different places: the repository, the toolchain, or the machine. That
/// is why they stay separate instead of collapsing into one "could not run".
#[derive(Debug)]
pub enum NotRun {
    /// A target path does not exist, or is not a regular file.
    ///
    /// A moved or deleted file must not read as a clean result.
    TargetMissing(PathBuf),
    /// A target exists but could not be read (permissions, I/O error, a broken LFS pointer).
    ///
    /// Distinct from [`NotRun::TargetMissing`] because "it is not there" and "it is there and I
    /// was not allowed to look" are different bugs with different fixes.
    Unreadable {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A required external program is not on `PATH`.
    ///
    /// The honest form of exit 127. Pattern matching is compiled in and cannot reach this, but
    /// the process helpers in [`crate::proc`] still spawn `git` and `cargo`, either of which can
    /// be missing from a given machine.
    ToolAbsent(String),
    /// A program ran and failed in a way that means the check did not complete — as opposed to
    /// completing and reporting a violation.
    ToolError {
        tool: String,
        status: i32,
        stderr: String,
    },
    /// A child died on a signal. **Never a `Failed`.**
    ///
    /// `ExitStatus::code()` is `None` here; synthesising `128+n` from the signal number would
    /// hand the next `match` arm a 137 that reads as an ordinary numeric failure. "The kernel
    /// shot the check" must not be reported as "the check found a problem".
    Signalled { tool: String, signal: i32 },
    /// A command exceeded its deadline. `crates/child_process` kills its whole process group.
    Timeout { tool: String, secs: u64 },
}

impl NotRun {
    /// The second line of the rendered failure — the one that names the cause.
    fn explain(&self, kind: Kind) -> String {
        let what = kind.noun();
        match self {
            NotRun::TargetMissing(_) => format!(
                "The {what} could not run. A moved or deleted file must not read as a clean result."
            ),
            NotRun::Unreadable { source, .. } => format!(
                "Unreadable: {source}. Refusing to report OK on a {what} that did not execute."
            ),
            NotRun::ToolAbsent(tool) => format!(
                "`{tool}` is ABSENT. Refusing to report OK on a {what} that did not execute."
            ),
            NotRun::ToolError { tool, status, .. } => format!(
                "`{tool}` exited {status} (read/pattern error). Refusing to report OK on a {what} that did not execute."
            ),
            NotRun::Signalled { tool, signal } => format!(
                "`{tool}` was killed by signal {signal} — the process died, it did not report. Refusing to report OK on a {what} that did not execute."
            ),
            NotRun::Timeout { tool, secs } => format!(
                "`{tool}` exceeded {secs}s and was killed. Refusing to report OK on a {what} that did not execute."
            ),
        }
    }

    /// The trailing clause on the headline: `— target file missing: <path>` and its siblings.
    fn headline_suffix(&self) -> String {
        match self {
            NotRun::TargetMissing(p) => format!(" — target file missing: {}", p.display()),
            NotRun::Unreadable { path, .. } => format!(" — unreadable target: {}", path.display()),
            NotRun::ToolAbsent(tool) => format!(" — {tool} not found"),
            NotRun::ToolError { tool, status, .. } => format!(" — {tool} exited {status}"),
            NotRun::Signalled { tool, signal } => format!(" — {tool} killed by signal {signal}"),
            NotRun::Timeout { tool, secs } => format!(" — {tool} timed out after {secs}s"),
        }
    }
}

/// A rendered failure: one headline plus zero or more six-space-indented continuation lines.
///
/// The six-space indent is shared by every gate, so a log reader can tell a continuation line
/// from the next headline at a glance.
#[derive(Debug, Clone)]
pub struct Finding {
    pub headline: String,
    pub detail: Vec<String>,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FAIL: {}", self.headline)?;
        for line in &self.detail {
            write!(f, "\n      {line}")?;
        }
        Ok(())
    }
}

/// The outcome of one check.
///
/// See the module docs. The `must_use` message is deliberately the failure mode it prevents.
#[must_use = "a Verdict that is never inspected is a check that never ran — match it or pass it to Report::check"]
#[derive(Debug)]
pub enum Verdict {
    /// The check ran and the invariant holds.
    Held,
    /// The check ran and found a violation.
    Failed(Finding),
    /// The check could not run. Structured cause kept alongside the rendered text so tests can
    /// assert on the *reason* rather than string-matching the prose.
    DidNotRun(NotRun, Finding),
}

impl Verdict {
    /// Build a `Failed` from a bare message, rendered as a plain `FAIL: <msg>`.
    pub fn failed(msg: impl Into<String>) -> Verdict {
        Verdict::Failed(Finding {
            headline: msg.into(),
            detail: Vec::new(),
        })
    }

    /// Build a `DidNotRun`, rendering the two-line form for the given cause and kind.
    pub fn did_not_run(msg: impl Into<String>, kind: Kind, cause: NotRun) -> Verdict {
        let finding = Finding {
            headline: format!("{}{}", msg.into(), cause.headline_suffix()),
            detail: vec![cause.explain(kind)],
        };
        Verdict::DidNotRun(cause, finding)
    }

    /// Three-outcome exit status: `Held` 0, `Failed` 1, `DidNotRun` **2**.
    ///
    /// Prefer this for new gates — a distinct code for "did not run" is the point of the whole
    /// exercise, and CI can then tell a real violation from a broken checkout.
    pub fn into_exit(self) -> i32 {
        match self {
            Verdict::Held => 0,
            Verdict::Failed(_) => 1,
            Verdict::DidNotRun(..) => 2,
        }
    }

    /// Collapses the three outcomes to exit 0/1: `Held` is 0, everything else is 1.
    ///
    /// **Only for gates whose callers already depend on a two-outcome exit contract.** Named the
    /// long way round so that choosing it is visible in review rather than a `!held()` nobody
    /// notices — it throws away the distinction between "failed" and "did not run". New code
    /// wants [`Verdict::into_exit`].
    pub fn into_binary_exit_code(self) -> i32 {
        match self {
            Verdict::Held => 0,
            Verdict::Failed(_) | Verdict::DidNotRun(..) => 1,
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verdict::Held => Ok(()),
            Verdict::Failed(finding) | Verdict::DidNotRun(_, finding) => finding.fmt(f),
        }
    }
}

#[cfg(test)]
#[path = "tests/verdict_tests.rs"]
mod tests;
