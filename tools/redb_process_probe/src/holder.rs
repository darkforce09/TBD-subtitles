//! The `hold` command: keeps a database open in this process until stdin reaches end of file,
//! optionally committing all the while.
//!
//! **Role:** the other process of every matrix scenario. The matrix starts this executable as
//! `hold`, waits for its `ready` line, makes its attempts, and closes the holder's stdin.
//!
//! **Position:** used by `main`; depends on `open_mode` and `redb`.
//!
//! **Signals and state:** stdin is the only control: end of file means close and exit. A
//! writing holder keeps each write transaction open about [`OPEN_TRANSACTION`] before it
//! commits, so an attempt from another process is likely to land while one is open.
//!
//! **Invariants:** `ready` is printed and flushed only after the open succeeded and, for a
//! read-write holder, after the counter row exists; a failed open, counter seed or write prints
//! `failed: <Display> | <Debug>` on stdout and exits 1.

use std::io::{self, Read, Write};
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use redb::{Database, ReadableTable};

use crate::open_mode::{COUNTER_KEY, COUNTER_TABLE, Handle, OpenMode, OpenOutcome, Sharing, open};

/// How long a writing holder keeps each write transaction open before it commits.
pub(crate) const OPEN_TRANSACTION: Duration = Duration::from_millis(20);

/// Runs `hold`: open, `ready`, hold (and write) until stdin ends, close, exit 0.
pub(crate) fn run(
    path: &Path,
    mode: OpenMode,
    sharing: Sharing,
    writing: bool,
) -> anyhow::Result<ExitCode> {
    if writing && mode == OpenMode::ReadOnly {
        anyhow::bail!("--writing needs --mode rw");
    }
    let (outcome, handle) = open(path, mode, sharing, None);
    let Some(handle) = handle else {
        if let OpenOutcome::Failed { display, debug, .. } = outcome {
            println!("failed: {display} | {debug}");
        }
        return Ok(ExitCode::FAILURE);
    };
    if let Handle::ReadWrite(db) = &handle
        && let Err(error) = seed_counter(db)
    {
        println!("failed: {error} | {error:?}");
        return Ok(ExitCode::FAILURE);
    }
    let mut stdout = io::stdout();
    writeln!(stdout, "ready")?;
    stdout.flush()?;

    match (&handle, writing) {
        (Handle::ReadWrite(db), true) => {
            let ended = Arc::new(AtomicBool::new(false));
            let watcher = {
                let ended = ended.clone();
                thread::spawn(move || {
                    let _ = io::copy(&mut io::stdin().lock(), &mut io::sink());
                    ended.store(true, Ordering::SeqCst);
                })
            };
            while !ended.load(Ordering::SeqCst) {
                if let Err(error) = increment_counter(db) {
                    println!("failed: {error} | {error:?}");
                    return Ok(ExitCode::FAILURE);
                }
            }
            let _ = watcher.join();
        }
        _ => {
            io::stdin().lock().read_to_end(&mut Vec::new())?;
        }
    }
    drop(handle);
    Ok(ExitCode::SUCCESS)
}

/// Creates the counter row with 0 when it is missing, so the database is not empty.
pub(crate) fn seed_counter(db: &Database) -> Result<(), redb::Error> {
    let transaction = db.begin_write()?;
    {
        let mut table = transaction.open_table(COUNTER_TABLE)?;
        if table.get(COUNTER_KEY)?.is_none() {
            table.insert(COUNTER_KEY, 0)?;
        }
    }
    transaction.commit()?;
    Ok(())
}

/// One write transaction: adds 1 to the counter, stays open [`OPEN_TRANSACTION`], commits.
fn increment_counter(db: &Database) -> Result<(), redb::Error> {
    let transaction = db.begin_write()?;
    {
        let mut table = transaction.open_table(COUNTER_TABLE)?;
        let next = table.get(COUNTER_KEY)?.map_or(0, |value| value.value()) + 1;
        table.insert(COUNTER_KEY, next)?;
    }
    thread::sleep(OPEN_TRANSACTION);
    transaction.commit()?;
    Ok(())
}
