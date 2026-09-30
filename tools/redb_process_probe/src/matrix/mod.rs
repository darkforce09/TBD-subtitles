//! The `matrix` command: every scenario of a second process opening a held database, as one
//! Markdown table.
//!
//! **Role:** prints the header (versions, feature, sharing mode, folder and filesystem), runs
//! the scenarios in order, prints the table, and forms the exit code.
//!
//! **Position:** used by `main`; the scenarios live in `scenarios`, the holder children in
//! `holder_process`, the table in `table`; depends on `attempt`, `filesystem` and `open_mode`.
//!
//! **Signals and state:** one database file per scenario in the folder, removed before the
//! scenario and after the matrix; holders are child processes of this executable, and every
//! attempt runs in the matrix process itself, which is a process of its own beside them.
//!
//! **Invariants:** a scenario that could not run (a holder that never became ready, a writing
//! holder that exited before or during its attempts, a crash that SIGKILL did not cause) is
//! recorded in the table and makes the exit code 2; any other outcome is data and the exit code
//! is 0.

mod holder_process;
mod scenarios;
mod table;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context;

use crate::attempt::{Attempt, attempt};
use crate::open_mode::{OpenMode, Sharing};
use crate::{REDB_VERSION, RKYV_VERSION, filesystem};

use self::holder_process::{HolderProcess, Readiness};
use self::table::Row;

/// Runs the matrix in `dir` under `sharing` and prints the header and the table.
pub(crate) fn run(dir: &Path, sharing: Sharing) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let dir = dir
        .canonicalize()
        .with_context(|| format!("resolve {}", dir.display()))?;
    println!("# redb process probe: matrix");
    println!();
    println!(
        "redb {REDB_VERSION}, rkyv {RKYV_VERSION}, multiprocess feature: {}, sharing: {}",
        if cfg!(feature = "multiprocess") {
            "on"
        } else {
            "off"
        },
        sharing.name()
    );
    println!("dir: {} on {}", dir.display(), filesystem::describe(&dir));
    println!(
        "holders are `hold` child processes of this executable; every attempt runs in the \
         matrix process, a separate process"
    );
    println!();

    let mut matrix = Matrix {
        dir,
        sharing,
        rows: Vec::new(),
        could_not_run: Vec::new(),
        files: Vec::new(),
    };
    matrix.idle_writer();
    matrix.busy_writer();
    matrix.reader();
    matrix.two_readers();
    matrix.crashed_writer();
    matrix.same_process();
    matrix.handover();
    for file in &matrix.files {
        let _ = std::fs::remove_file(file);
    }

    print!("{}", table::render(&matrix.rows));
    if matrix.could_not_run.is_empty() {
        return Ok(ExitCode::SUCCESS);
    }
    println!();
    for reason in &matrix.could_not_run {
        println!("could not run: {reason}");
    }
    Ok(ExitCode::from(2))
}

/// The state of one matrix run: where its files are and what it has recorded.
struct Matrix {
    dir: PathBuf,
    sharing: Sharing,
    rows: Vec<Row>,
    could_not_run: Vec<String>,
    files: Vec<PathBuf>,
}

impl Matrix {
    /// A database path for `name` with no file behind it; removed again at the end.
    fn fresh(&mut self, name: &str) -> PathBuf {
        let path = self.dir.join(format!("scenario-{name}.redb"));
        let _ = std::fs::remove_file(&path);
        self.files.push(path.clone());
        path
    }

    /// Records one observation.
    fn row(&mut self, scenario: &str, holder: &str, attempt: &str, result: impl Into<String>) {
        self.rows.push(Row {
            scenario: scenario.to_string(),
            holder: holder.to_string(),
            attempt: attempt.to_string(),
            result: result.into(),
        });
    }

    /// Starts a holder and waits for `ready`; records the failure and returns `None` otherwise.
    fn start_holder(
        &mut self,
        scenario: &str,
        holder: &str,
        path: &Path,
        mode: OpenMode,
        writing: bool,
    ) -> Option<HolderProcess> {
        let process = match HolderProcess::spawn(path, mode, self.sharing, writing) {
            Ok(process) => process,
            Err(error) => {
                self.row(scenario, holder, "start holder", format!("{error:#}"));
                self.could_not_run
                    .push(format!("scenario {scenario}: {holder}: {error:#}"));
                return None;
            }
        };
        match process.wait_ready() {
            Readiness::Ready => Some(process),
            Readiness::NotReady(line) => {
                let ended = process.close();
                self.row(
                    scenario,
                    holder,
                    "start holder",
                    format!("not ready: {line}; {ended}"),
                );
                self.could_not_run
                    .push(format!("scenario {scenario}: {holder} not ready: {line}"));
                None
            }
        }
    }

    /// One attempt from this process, recorded under its mode's name.
    fn attempt_row(&mut self, scenario: &str, holder: &str, path: &Path, mode: OpenMode) {
        let result = attempt(path, mode, self.sharing, false);
        self.row(scenario, holder, mode.name(), result.render());
    }

    /// `count` attempts from this process, a few milliseconds apart, recorded as one summary.
    fn repeated_attempts(&mut self, path: &Path, mode: OpenMode, count: usize) -> Vec<Attempt> {
        (0..count)
            .map(|_| {
                let result = attempt(path, mode, self.sharing, false);
                std::thread::sleep(std::time::Duration::from_millis(5));
                result
            })
            .collect()
    }
}
