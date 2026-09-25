//! Accumulating many checks into one exit status.
//!
//! **Role:** [`Report`], which accumulates verdicts, prints each failure as it lands and yields
//! the exit status.
//!
//! **Position:** used by every gate at the end of a run.
//!
//! **Signals and state:** counts of checks run, failed and not run; prints to stdout.
//!
//! **Invariants:** exit 0 only when every check ran and held; a check that did not run outranks a
//! violation (2 over 1).
//!
//! A gate that stopped at its first violation would report one problem per run, so every gate
//! accumulates instead. [`Report`] is that accumulation, written once: one counter, one summary
//! line, one exit contract, so no two gates disagree on the wording or on what a mixed run means.
//!
//! It keeps the two failure kinds apart in the summary, because "3 violations" and "3 checks that
//! never executed" call for completely different next actions, and a merged count hides the
//! second behind the first.

use crate::verdict::Verdict;

/// Accumulates verdicts, prints each failure as it lands, and yields one exit status.
pub struct Report {
    label: String,
    ran: u32,
    failed: u32,
    did_not_run: u32,
}

impl Report {
    pub fn new(label: impl Into<String>) -> Report {
        Report {
            label: label.into(),
            ran: 0,
            failed: 0,
            did_not_run: 0,
        }
    }

    /// Record one verdict, printing its rendered failure immediately so output order follows
    /// check order even when a later check is slow.
    pub fn check(&mut self, verdict: Verdict) -> &mut Report {
        self.ran += 1;
        match verdict {
            Verdict::Held => {}
            Verdict::Failed(ref f) => {
                self.failed += 1;
                println!("{f}");
            }
            Verdict::DidNotRun(_, ref f) => {
                self.did_not_run += 1;
                println!("{f}");
            }
        }
        self
    }

    /// True only when every check ran AND held.
    pub fn clean(&self) -> bool {
        self.failed == 0 && self.did_not_run == 0
    }

    /// Print the summary and yield the three-outcome exit status.
    ///
    /// A check that did not run outranks a violation: if some inputs were never examined, the
    /// "0 violations" over the rest is not a result anyone should act on.
    pub fn finish(self) -> i32 {
        if self.clean() {
            println!("\n{}: OK — {} check(s), all held", self.label, self.ran);
            return 0;
        }
        if self.did_not_run > 0 {
            eprintln!(
                "\n{}: FAIL — {} violation(s), {} check(s) DID NOT RUN (of {})",
                self.label, self.failed, self.did_not_run, self.ran
            );
            eprintln!(
                "  A check that did not run is not a pass. Fix the missing input or absent tool"
            );
            eprintln!("  before reading anything into the checks that did complete.");
            return 2;
        }
        eprintln!(
            "\n{}: FAIL — {} violation(s) of {}",
            self.label, self.failed, self.ran
        );
        1
    }

    /// Summary counts as `(ran, failed, did_not_run)`, for tests and for callers that map onto a
    /// two-outcome exit contract.
    pub fn counts(&self) -> (u32, u32, u32) {
        (self.ran, self.failed, self.did_not_run)
    }
}

#[cfg(test)]
#[path = "tests/report_tests.rs"]
mod tests;
