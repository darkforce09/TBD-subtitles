//! The `redb-process-probe` binary: proves how redb behaves when a second process opens a
//! database file that another process holds open or is writing, and that an rkyv archive reads
//! in place from a redb value slice, and measures the RAM one large write transaction holds.
//!
//! **Role:** the command line and the dispatch to `hold`, `attempt`, `matrix`, `inplace` and
//! `txn-memory`.
//!
//! **Position:** a repository tool; depends on `redb`, `rkyv`, `clap` and `anyhow`, and on no
//! workspace crate. `matrix` starts this same executable again as `hold` child processes.
//!
//! **Signals and state:** reads and writes only the database files it is pointed at, and
//! `/proc/self/mountinfo` to name the filesystem and `/proc/self/status` for `txn-memory`.
//!
//! **Invariants:** `hold` prints exactly `ready` once its database is open and exits when stdin
//! reaches end of file; `attempt` exits 0 whatever the open did, because the outcome is the
//! data; `matrix` exits 0 when every scenario ran and 2 when one could not; `inplace` exits 1
//! when a step of its check fails and 2 when it cannot run (its folder cannot be created), and so
//! does `txn-memory` for its measurement; a refused open is reported with redb's own Display and Debug texts, never paraphrased.

mod attempt;
mod filesystem;
mod holder;
mod inplace;
mod matrix;
mod open_mode;
mod txn_memory;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::open_mode::{OpenMode, Sharing};

/// The redb version the manifest pins exactly; the output names it.
pub(crate) const REDB_VERSION: &str = "4.3.0";

/// The rkyv version the manifest asks for; the output names it.
pub(crate) const RKYV_VERSION: &str = "0.8.18";

/// How redb behaves when a second process opens the same database file.
#[derive(Debug, Parser)]
#[command(name = "redb-process-probe", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Open the database, print `ready`, and keep it open until stdin reaches end of file.
    Hold {
        /// The database file; `--mode rw` creates it when missing.
        #[arg(long)]
        db: PathBuf,
        /// `rw` opens it for writing, `ro` read-only.
        #[arg(long)]
        mode: OpenMode,
        /// Keep committing: each write transaction stays open about 20 ms before its commit.
        #[arg(long)]
        writing: bool,
        /// How processes share the file; needs the `multiprocess` feature for anything but the
        /// exclusive writer.
        #[arg(long)]
        sharing: Option<Sharing>,
    },
    /// Open the database once and print how the open went, in one line.
    Attempt {
        /// The database file.
        #[arg(long)]
        db: PathBuf,
        /// `rw` opens it for writing (creating it when missing), `ro` read-only.
        #[arg(long)]
        mode: OpenMode,
        /// How processes share the file.
        #[arg(long)]
        sharing: Option<Sharing>,
    },
    /// Run every scenario against fresh database files in the folder and print a table.
    Matrix {
        /// The folder for the database files; created when missing.
        #[arg(long)]
        dir: PathBuf,
        /// How processes share the file, for the holders and the attempts alike.
        #[arg(long)]
        sharing: Option<Sharing>,
    },
    /// Store an rkyv archive in redb, reopen it and read the archive in place from the value.
    Inplace {
        /// The folder for the database file; created when missing.
        #[arg(long)]
        dir: PathBuf,
    },
    /// Fill one large write transaction with per-frame rows and print the memory redb holds.
    TxnMemory(txn_memory::Options),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("redb-process-probe: {error:#}");
            ExitCode::from(2)
        }
    }
}

fn run(command: Command) -> anyhow::Result<ExitCode> {
    match command {
        Command::Hold {
            db,
            mode,
            writing,
            sharing,
        } => holder::run(&db, mode, Sharing::resolve(sharing)?, writing),
        Command::Attempt { db, mode, sharing } => {
            let result = attempt::attempt(&db, mode, Sharing::resolve(sharing)?, false);
            println!("{}", result.render());
            Ok(ExitCode::SUCCESS)
        }
        Command::Matrix { dir, sharing } => matrix::run(&dir, Sharing::resolve(sharing)?),
        Command::Inplace { dir } => match inplace::run(inplace::prepare(&dir)?) {
            Ok(lines) => {
                for line in lines {
                    println!("{line}");
                }
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => {
                println!("inplace: failed: {error:#}");
                Ok(ExitCode::FAILURE)
            }
        },
        Command::TxnMemory(options) => txn_memory::command(&options),
    }
}
