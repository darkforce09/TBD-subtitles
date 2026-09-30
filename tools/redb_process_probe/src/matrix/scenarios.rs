//! The seven matrix scenarios, in the order the table lists them.
//!
//! **Role:** each scenario sets up a holder (or none), makes its attempts from the matrix
//! process, ends the holder, and records every observation as a row.
//!
//! **Position:** methods of the matrix in `mod.rs`; depends on `attempt`, `holder` (the counter
//! seed) and `open_mode`.
//!
//! **Signals and state:** each scenario has a database file of its own, seeded before a
//! read-only holder needs it.
//!
//! **Invariants:** a holder is always ended (stdin closed, or SIGKILL in the crash scenario)
//! and waited for before its scenario returns. A writing holder is checked to be running before
//! and after the attempts made against it and right before its SIGKILL; one that has already
//! exited is recorded as `holder exited early: …` and makes the matrix exit 2, and a crash
//! counts only when SIGKILL is what ended the holder.

use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::attempt::{attempt, summarize};
use crate::holder::seed_counter;
use crate::open_mode::{OpenMode, builder};

use super::{HolderProcess, Matrix};

/// How many attempts the busy-writer scenario makes per mode.
const BUSY_ATTEMPTS: usize = 20;

/// How long a writing holder runs before the crash scenario kills it.
const RUN_BEFORE_KILL: Duration = Duration::from_millis(300);

impl Matrix {
    /// Scenario 1: an idle read-write holder; attempts rw, then ro.
    pub(super) fn idle_writer(&mut self) {
        let (scenario, holder) = ("1", "rw, idle");
        let path = self.fresh("1");
        let Some(process) = self.start_holder(scenario, holder, &path, OpenMode::ReadWrite, false)
        else {
            return;
        };
        self.attempt_row(scenario, holder, &path, OpenMode::ReadWrite);
        self.attempt_row(scenario, holder, &path, OpenMode::ReadOnly);
        let ended = process.close();
        self.row(scenario, holder, "close holder", ended);
    }

    /// Scenario 2: a read-write holder committing all the while; 20 attempts rw, then 20 ro.
    pub(super) fn busy_writer(&mut self) {
        let (scenario, holder) = ("2", "rw, writing (each txn open 20 ms)");
        let path = self.fresh("2");
        let Some(mut process) =
            self.start_holder(scenario, holder, &path, OpenMode::ReadWrite, true)
        else {
            return;
        };
        for mode in [OpenMode::ReadWrite, OpenMode::ReadOnly] {
            let label = format!("{} ×{BUSY_ATTEMPTS}", mode.name());
            if !self.holder_running(scenario, holder, &mut process, &format!("before {label}")) {
                return;
            }
            let attempts = self.repeated_attempts(&path, mode, BUSY_ATTEMPTS);
            if !self.holder_running(scenario, holder, &mut process, &format!("after {label}")) {
                let summary = summarize(&attempts);
                self.row(scenario, holder, &label, format!("not credited: {summary}"));
                return;
            }
            self.row(scenario, holder, &label, summarize(&attempts));
        }
        let ended = process.close();
        self.row(scenario, holder, "close holder", ended);
    }

    /// Scenario 3: a read-only holder; attempts ro, then rw.
    pub(super) fn reader(&mut self) {
        let (scenario, holder) = ("3", "ro");
        let path = self.fresh("3");
        if !self.seed(scenario, holder, &path) {
            return;
        }
        let Some(process) = self.start_holder(scenario, holder, &path, OpenMode::ReadOnly, false)
        else {
            return;
        };
        self.attempt_row(scenario, holder, &path, OpenMode::ReadOnly);
        self.attempt_row(scenario, holder, &path, OpenMode::ReadWrite);
        let ended = process.close();
        self.row(scenario, holder, "close holder", ended);
    }

    /// Scenario 4: two read-only holders at once, the second started while the first is up.
    pub(super) fn two_readers(&mut self) {
        let scenario = "4";
        let path = self.fresh("4");
        if !self.seed(scenario, "ro + ro", &path) {
            return;
        }
        let Some(first) = self.start_holder(scenario, "ro", &path, OpenMode::ReadOnly, false)
        else {
            return;
        };
        self.row(scenario, "ro", "first ro holder", "ready");
        match HolderProcess::spawn(&path, OpenMode::ReadOnly, self.sharing, false) {
            Ok(second) => {
                let readiness = second.wait_ready();
                let result = match &readiness {
                    super::Readiness::Ready => "ready".to_string(),
                    super::Readiness::NotReady(line) => format!("not ready: {line}"),
                };
                self.row(scenario, "ro (first still up)", "second ro holder", result);
                let ended = second.close();
                self.row(
                    scenario,
                    "ro (first still up)",
                    "close second holder",
                    ended,
                );
            }
            Err(error) => {
                self.row(scenario, "ro", "second ro holder", format!("{error:#}"));
                self.could_not_run
                    .push(format!("scenario 4: second holder: {error:#}"));
            }
        }
        let ended = first.close();
        self.row(scenario, "ro", "close first holder", ended);
    }

    /// Scenario 5: a writing holder killed by SIGKILL; a rw open repairs it, and on a second
    /// killed database a ro open is tried first.
    pub(super) fn crashed_writer(&mut self) {
        let scenario = "5";
        let holder = "rw, writing, SIGKILL after 300 ms";
        let path = self.fresh("5a");
        if !self.crash(scenario, holder, &path) {
            return;
        }
        let repaired = attempt(&path, OpenMode::ReadWrite, self.sharing, true);
        self.row(
            scenario,
            holder,
            "rw with repair callback",
            repaired.render(),
        );
        let after = attempt(&path, OpenMode::ReadOnly, self.sharing, false);
        self.row(
            scenario,
            "killed, then repaired by the rw open above",
            "ro",
            after.render(),
        );

        let second = self.fresh("5b");
        let holder = "fresh db: rw, writing, SIGKILL after 300 ms";
        if !self.crash(scenario, holder, &second) {
            return;
        }
        let read_only = attempt(&second, OpenMode::ReadOnly, self.sharing, false);
        self.row(scenario, holder, "ro first", read_only.render());
        let repaired = attempt(&second, OpenMode::ReadWrite, self.sharing, true);
        self.row(
            scenario,
            holder,
            "then rw with repair callback",
            repaired.render(),
        );
    }

    /// Scenario 6: a rw handle in this process; a second rw open and a ro open beside it.
    pub(super) fn same_process(&mut self) {
        let (scenario, holder) = ("6", "rw handle in the matrix process itself");
        let path = self.fresh("6");
        let first = builder(self.sharing, None).create(&path);
        let db = match first {
            Ok(db) => db,
            Err(error) => {
                self.row(
                    scenario,
                    holder,
                    "first rw open",
                    format!("failed: {error} | {error:?}"),
                );
                self.could_not_run
                    .push(format!("scenario 6: first rw open: {error}"));
                return;
            }
        };
        if let Err(error) = seed_counter(&db) {
            self.row(scenario, holder, "seed", format!("{error} | {error:?}"));
        }
        self.attempt_row(scenario, holder, &path, OpenMode::ReadWrite);
        self.attempt_row(scenario, holder, &path, OpenMode::ReadOnly);
        drop(db);
    }

    /// Scenario 7: a rw holder that exits cleanly first; attempts rw, then ro.
    pub(super) fn handover(&mut self) {
        let (scenario, holder) = ("7", "rw, exited cleanly before the attempts");
        let path = self.fresh("7");
        let Some(process) = self.start_holder(scenario, holder, &path, OpenMode::ReadWrite, false)
        else {
            return;
        };
        let ended = process.close();
        self.row(scenario, holder, "close holder", ended);
        self.attempt_row(scenario, holder, &path, OpenMode::ReadWrite);
        self.attempt_row(scenario, holder, &path, OpenMode::ReadOnly);
    }

    /// Creates the database with its counter row in this process and closes it again.
    fn seed(&mut self, scenario: &str, holder: &str, path: &Path) -> bool {
        let seeded = builder(self.sharing, None)
            .create(path)
            .map_err(redb::Error::from)
            .and_then(|db| seed_counter(&db));
        match seeded {
            Ok(()) => true,
            Err(error) => {
                self.row(scenario, holder, "seed", format!("{error} | {error:?}"));
                self.could_not_run
                    .push(format!("scenario {scenario}: seed: {error}"));
                false
            }
        }
    }

    /// Starts a writing holder, lets it run, kills it and records how it ended; `false` when
    /// the holder was not running at the kill, or SIGKILL is not what ended it.
    fn crash(&mut self, scenario: &str, holder: &str, path: &Path) -> bool {
        let Some(mut process) =
            self.start_holder(scenario, holder, path, OpenMode::ReadWrite, true)
        else {
            return false;
        };
        if !self.holder_running(scenario, holder, &mut process, "after ready") {
            return false;
        }
        thread::sleep(RUN_BEFORE_KILL);
        if !self.holder_running(scenario, holder, &mut process, "before SIGKILL") {
            return false;
        }
        match process.kill() {
            Ok(ended) => {
                self.row(scenario, holder, "SIGKILL holder", ended);
                true
            }
            Err(ended) => {
                self.row(scenario, holder, "SIGKILL holder", ended.clone());
                self.could_not_run
                    .push(format!("scenario {scenario}: {holder}: SIGKILL: {ended}"));
                false
            }
        }
    }

    /// Whether the holder still runs at `when`; when it has exited, records how, as a row and
    /// as a reason the matrix could not run, so no result is credited to a live holder.
    fn holder_running(
        &mut self,
        scenario: &str,
        holder: &str,
        process: &mut HolderProcess,
        when: &str,
    ) -> bool {
        let Some(ended) = process.exited() else {
            return true;
        };
        self.row(
            scenario,
            holder,
            when,
            format!("holder exited early: {ended}"),
        );
        self.could_not_run.push(format!(
            "scenario {scenario}: {holder}: holder exited early ({when}): {ended}"
        ));
        false
    }
}
