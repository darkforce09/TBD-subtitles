//! `tbd-subtitles dump <job_or_video> <table> [key]`: a job database's rows as JSON.
//!
//! **Role:** find the job a folder, a job name or a video names, open its `job.redb` if no other
//! process owns it, and print one row pretty-printed, or every row of a table as JSON Lines, each
//! through its record kind.
//!
//! **Position:** called by `cli::dispatch`; resolves the work root as `process` does, through
//! `settings::services`; reads the rows through `pipeline::work_dir::JobStore` and prints them
//! through `pipeline::work_dir::store::kinds`.
//!
//! **Signals and state:** reads the settings file; opens the job's database, which writes
//! `job.lock` while it is open, as every owner does; prints to stdout.
//!
//! **Invariants:** it never creates a job or a database; a job another process owns is refused
//! with that process's pid, never opened beside it; a row without a record kind prints its size,
//! never its bytes; the exit code is 0 when it printed, 1 for a missing job or row or a busy job,
//! and 2 on a usage error.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context;
use clap::Args;
use pipeline::work_dir::store::{StoreRead, kind, kinds};
use pipeline::work_dir::{JobStore, WorkDir, job_id};
use serde_json::{Value, json};
use worker_channel::address::{Key, Table};

use crate::settings::services::{job_settings, settings_file};

/// The options of one `dump` run.
#[derive(Debug, Args)]
pub(super) struct DumpArgs {
    /// The job: its work directory, its folder name under the work root, or its video.
    pub(super) job_or_video: PathBuf,
    /// The table: `meta`, `step_records`, `outputs`, `corrections`, `frames` or `readings`.
    #[arg(value_parser = parse_table)]
    pub(super) table: Table,
    /// One row's key: a name such as `probe_decode`, or `<occurrence>/<frame>` in `frames` and
    /// `readings`. Without it, every row of the table prints as JSON Lines.
    pub(super) key: Option<String>,
    /// The settings file that names the work root; default:
    /// `~/.config/tbd-subtitles/settings.toml`.
    #[arg(long)]
    settings: Option<PathBuf>,
    /// The folder that holds the jobs' work directories; wins over the settings file.
    #[arg(long)]
    work_root: Option<PathBuf>,
}

/// A table named on the command line.
fn parse_table(text: &str) -> Result<Table, String> {
    text.parse()
}

/// `text` as a key of `table`: a name, or `<occurrence>/<frame>` in the per-frame tables.
pub(super) fn parse_key(table: Table, text: &str) -> Result<Key, String> {
    match table {
        Table::Frames | Table::Readings => {
            let parsed = text.rsplit_once('/').and_then(|(occurrence, frame)| {
                let frame = frame.parse::<u64>().ok()?;
                (!occurrence.is_empty()).then(|| Key::Frame {
                    occurrence: occurrence.to_string(),
                    frame,
                })
            });
            parsed.ok_or_else(|| {
                format!("`{text}` is no key of the {table} table: write <occurrence>/<frame>")
            })
        }
        _ if text.is_empty() => Err(format!("an empty key names no row of the {table} table")),
        _ => Ok(Key::Name(text.to_string())),
    }
}

/// The work directory of the job `job_or_video` names: a folder that holds `job.redb`, a folder
/// under `work_root`, or a video, whose job folder is named from its canonical path.
pub(super) fn resolve(job_or_video: &Path, work_root: &Path) -> anyhow::Result<WorkDir> {
    if job_or_video.is_dir() && WorkDir::new(job_or_video).database().is_file() {
        return Ok(WorkDir::new(job_or_video));
    }
    let named = work_root.join(job_or_video);
    if job_or_video.is_relative() && named.is_dir() {
        return Ok(WorkDir::new(named));
    }
    if job_or_video.is_file() {
        let video = std::fs::canonicalize(job_or_video)
            .with_context(|| format!("cannot find {}", job_or_video.display()))?;
        return Ok(WorkDir::new(work_root.join(job_id(&video))));
    }
    anyhow::bail!(
        "{} is no job folder, no job under {} and no video",
        job_or_video.display(),
        work_root.display()
    )
}

/// Print the row of `key` in `table` pretty-printed, or every row of `table` as JSON Lines when
/// there is no key; `false` when the row is missing.
pub(super) fn print(
    read: &StoreRead,
    table: Table,
    key: Option<&Key>,
    out: &mut impl Write,
) -> anyhow::Result<bool> {
    match key {
        Some(key) => {
            let Some(bytes) = read.raw(table, key)? else {
                return Ok(false);
            };
            let shown = match kind(table, key) {
                Ok(kind) => kind.json(&bytes)?,
                Err(_) => row(table, key, &bytes),
            };
            serde_json::to_writer_pretty(&mut *out, &shown)?;
            writeln!(out)?;
        }
        None => {
            for key in read.keys(table)? {
                let bytes = read.raw(table, &key)?.unwrap_or_default();
                serde_json::to_writer(&mut *out, &row(table, &key, &bytes))?;
                writeln!(out)?;
            }
        }
    }
    Ok(true)
}

/// One row as `{"key", "value"}`; a row with no record kind, or one whose archive does not
/// read, has a null value and its size, and the reason when it does not read.
fn row(table: Table, key: &Key, bytes: &[u8]) -> Value {
    let shown = kinds::shown(key);
    match kind(table, key).map(|kind| kind.json(bytes)) {
        Ok(Ok(value)) => json!({ "key": shown, "value": value }),
        Ok(Err(error)) => json!({
            "key": shown,
            "value": null,
            "bytes": bytes.len(),
            "error": error.to_string(),
        }),
        Err(_) => json!({ "key": shown, "value": null, "bytes": bytes.len() }),
    }
}

/// Print what the options ask for; the exit code.
pub(super) fn run(args: &DumpArgs) -> anyhow::Result<ExitCode> {
    let key = match args.key.as_deref().map(|text| parse_key(args.table, text)) {
        None => None,
        Some(Ok(key)) => Some(key),
        Some(Err(usage)) => {
            eprintln!("tbd-subtitles: {usage}");
            return Ok(ExitCode::from(2));
        }
    };
    let path = match &args.settings {
        Some(path) => path.clone(),
        None => settings_file::default_path()?,
    };
    let mut chosen = settings_file::load(&path)?;
    if let Some(root) = &args.work_root {
        chosen.work_root = Some(root.clone());
    }
    let work = resolve(&args.job_or_video, &job_settings::work_root(&chosen)?)?;
    anyhow::ensure!(
        work.database().is_file(),
        "the job {} has no job database, {}",
        work.root().display(),
        work.database().display()
    );
    let store = JobStore::open_existing(&work).map_err(|error| match error.busy_owner() {
        Some(Some(pid)) => {
            anyhow::anyhow!("process {pid} is running this job; dump it after that run ends")
        }
        Some(None) => {
            anyhow::anyhow!("another process is running this job; dump it after that run ends")
        }
        None => anyhow::Error::new(error),
    })?;
    let read = store.read()?;
    let mut out = std::io::stdout().lock();
    if print(&read, args.table, key.as_ref(), &mut out)? {
        return Ok(ExitCode::SUCCESS);
    }
    anyhow::bail!(
        "the job {} has no row {} in the {} table",
        work.root().display(),
        args.key.as_deref().unwrap_or_default(),
        args.table
    )
}

#[cfg(test)]
#[path = "tests/dump_command.rs"]
mod tests;
