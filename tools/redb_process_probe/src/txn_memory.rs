//! The `txn-memory` command: how much RAM redb holds for one large uncommitted write
//! transaction.
//!
//! **Role:** fills one write transaction with rows shaped like a per-frame table (a key of an
//! occurrence id and a frame number, a value of incompressible bytes), samples the process's
//! resident and peak resident memory while it grows, through the commit and after the database
//! is dropped, and times the inserts and the commit.
//!
//! **Position:** used by `main`; depends on `open_mode` for the builder, on `redb` and `clap`,
//! and reads `/proc/self/status`.
//!
//! **Signals and state:** writes one database file, `<dir>/txn-memory.redb`, removed first when
//! present and removed again at the end, whether the run held or not, unless `--keep`.
//!
//! **Invariants:** every row goes into the one write transaction, which commits once after the
//! last insert; each value is filled in place through `insert_reserve`, never built separately;
//! memory is read from the kernel's `VmRSS` and `VmHWM`, never estimated.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use anyhow::Context;
use redb::TableDefinition;

use crate::open_mode::{Sharing, builder};

/// The per-frame table: `(occurrence id, frame number)` to the frame's bytes.
const FRAME_TABLE: TableDefinition<(&str, u64), &[u8]> = TableDefinition::new("frames");

/// The database file's name inside `--dir`.
const DATABASE_FILE: &str = "txn-memory.redb";

/// Bytes in a mebibyte.
const MIB: f64 = 1024.0 * 1024.0;

/// The command's options.
#[derive(Debug, Clone, clap::Args)]
pub(crate) struct Options {
    /// The folder for the database file `txn-memory.redb`; created when missing.
    #[arg(long)]
    pub(crate) dir: PathBuf,
    /// Rows inserted in the one write transaction; the default is two hours at 60 fps.
    #[arg(long, default_value_t = 432_000, value_parser = clap::value_parser!(u64).range(1..))]
    pub(crate) rows: u64,
    /// Bytes in each row's value.
    #[arg(long, default_value_t = 2048, value_parser = clap::value_parser!(u64).range(1..))]
    pub(crate) value_bytes: u64,
    /// redb's page cache in MiB (redb's default is 1024).
    #[arg(long, default_value_t = 1024)]
    pub(crate) cache_mib: u64,
    /// Occurrence ids the rows are spread over, row by row.
    #[arg(long, default_value_t = 40, value_parser = clap::value_parser!(u64).range(1..))]
    pub(crate) occurrences: u64,
    /// Leave the database file in place at the end.
    #[arg(long)]
    pub(crate) keep: bool,
}

/// The process's resident and peak resident memory, in kB as the kernel reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MemoryStatus {
    pub(crate) rss_kb: u64,
    pub(crate) hwm_kb: u64,
}

/// One memory reading at a point of the run.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sample {
    /// Rows inserted so far.
    pub(crate) rows: u64,
    pub(crate) rss_mib: f64,
    pub(crate) hwm_mib: f64,
    /// Where in the run the reading was taken.
    pub(crate) stage: &'static str,
}

impl Sample {
    /// `rows <n> rss <MiB> hwm <MiB> (<stage>)`.
    pub(crate) fn render(&self) -> String {
        format!(
            "rows {} rss {:.1} hwm {:.1} ({})",
            self.rows, self.rss_mib, self.hwm_mib, self.stage
        )
    }
}

/// What one run measured.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Summary {
    pub(crate) database: PathBuf,
    pub(crate) rows: u64,
    pub(crate) value_bytes: u64,
    pub(crate) payload_mib: f64,
    pub(crate) cache_mib: u64,
    pub(crate) insert_seconds: f64,
    pub(crate) commit_seconds: f64,
    pub(crate) file_mib: f64,
    pub(crate) peak_hwm_mib: f64,
    pub(crate) rss_before_commit_mib: f64,
    pub(crate) rss_after_commit_mib: f64,
    pub(crate) kept: bool,
}

impl Summary {
    /// The summary block, one `name: value` line each.
    pub(crate) fn render(&self) -> Vec<String> {
        vec![
            "summary:".to_string(),
            format!("  database: {}", self.database.display()),
            format!("  rows: {}", self.rows),
            format!("  value bytes: {}", self.value_bytes),
            format!("  payload MiB: {:.1}", self.payload_mib),
            format!("  cache MiB: {}", self.cache_mib),
            format!("  insert s: {:.2}", self.insert_seconds),
            format!("  commit s: {:.2}", self.commit_seconds),
            format!("  file MiB: {:.1}", self.file_mib),
            format!("  peak hwm MiB: {:.1}", self.peak_hwm_mib),
            format!("  rss before commit MiB: {:.1}", self.rss_before_commit_mib),
            format!("  rss after commit MiB: {:.1}", self.rss_after_commit_mib),
            format!(
                "  database file: {}",
                if self.kept { "kept" } else { "removed" }
            ),
        ]
    }
}

/// Runs the command: prints each sample as it is taken, then the summary. Exits 1 when the
/// measurement fails, printed as `txn-memory: failed: <error>`; an error (exit 2) means it could
/// not start, such as a folder that cannot be created.
pub(crate) fn command(options: &Options) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&options.dir)
        .with_context(|| format!("create {}", options.dir.display()))?;
    match run(options, &mut |sample| println!("{}", sample.render())) {
        Ok(summary) => {
            for line in summary.render() {
                println!("{line}");
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            println!("txn-memory: failed: {error:#}");
            Ok(ExitCode::FAILURE)
        }
    }
}

/// Measures one run in `options.dir`, which must exist, handing each sample to `on_sample` as it
/// is taken. The database file is removed at the end unless `options.keep`.
pub(crate) fn run(
    options: &Options,
    on_sample: &mut dyn FnMut(&Sample),
) -> anyhow::Result<Summary> {
    let path = options.dir.join(DATABASE_FILE);
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    let result = measure(options, &path, on_sample);
    if !options.keep {
        let _ = std::fs::remove_file(&path);
    }
    result
}

fn measure(
    options: &Options,
    path: &Path,
    on_sample: &mut dyn FnMut(&Sample),
) -> anyhow::Result<Summary> {
    let value_bytes = usize::try_from(options.value_bytes).context("--value-bytes")?;
    let cache_bytes = options
        .cache_mib
        .checked_mul(1024 * 1024)
        .and_then(|bytes| usize::try_from(bytes).ok())
        .context("--cache-mib is too large")?;
    let mut samples = Vec::new();
    let mut take = |rows: u64, stage: &'static str| -> anyhow::Result<Sample> {
        let status = read_memory_status()?;
        let sample = Sample {
            rows,
            rss_mib: status.rss_kb as f64 / 1024.0,
            hwm_mib: status.hwm_kb as f64 / 1024.0,
            stage,
        };
        on_sample(&sample);
        samples.push(sample.clone());
        Ok(sample)
    };

    let mut builder = builder(Sharing::Exclusive, None);
    builder.set_cache_size(cache_bytes);
    let db = builder
        .create(path)
        .with_context(|| format!("create {}", path.display()))?;
    take(0, "before transaction")?;
    let checkpoints = checkpoints(options.rows);
    let transaction = db.begin_write()?;
    let insert_start = Instant::now();
    {
        let mut table = transaction.open_table(FRAME_TABLE)?;
        let mut next_checkpoint = checkpoints.iter().peekable();
        for frame in 0..options.rows {
            let occurrence = format!("occ-{:04}", frame % options.occurrences);
            let mut guard = table.insert_reserve((occurrence.as_str(), frame), value_bytes)?;
            fill_pseudo_random(guard.as_mut(), frame);
            drop(guard);
            let inserted = frame + 1;
            if next_checkpoint.next_if_eq(&&inserted).is_some() {
                take(inserted, "inserting")?;
            }
        }
    }
    let insert_seconds = insert_start.elapsed().as_secs_f64();
    let before_commit = take(options.rows, "before commit")?;
    let commit_start = Instant::now();
    transaction.commit()?;
    let commit_seconds = commit_start.elapsed().as_secs_f64();
    let after_commit = take(options.rows, "after commit")?;
    let file_bytes = std::fs::metadata(path)
        .with_context(|| format!("stat {}", path.display()))?
        .len();
    drop(db);
    take(options.rows, "after drop")?;

    let peak_hwm_mib = samples
        .iter()
        .map(|sample| sample.hwm_mib)
        .fold(0.0, f64::max);
    Ok(Summary {
        database: path.to_path_buf(),
        rows: options.rows,
        value_bytes: options.value_bytes,
        payload_mib: (options.rows as f64) * (options.value_bytes as f64) / MIB,
        cache_mib: options.cache_mib,
        insert_seconds,
        commit_seconds,
        file_mib: file_bytes as f64 / MIB,
        peak_hwm_mib,
        rss_before_commit_mib: before_commit.rss_mib,
        rss_after_commit_mib: after_commit.rss_mib,
        kept: options.keep,
    })
}

/// The row counts after each tenth of `rows`, short of the last row, ascending and distinct.
pub(crate) fn checkpoints(rows: u64) -> Vec<u64> {
    let mut points: Vec<u64> = (1..10)
        .map(|tenth| rows * tenth / 10)
        .filter(|&point| point > 0 && point < rows)
        .collect();
    points.dedup();
    points
}

/// Fills `bytes` with xorshift64 output seeded from `row`, so every row differs and nothing
/// compresses.
pub(crate) fn fill_pseudo_random(bytes: &mut [u8], row: u64) {
    let mut state = row.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    for chunk in bytes.chunks_mut(8) {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        chunk.copy_from_slice(&state.to_le_bytes()[..chunk.len()]);
    }
}

/// Reads `VmRSS` and `VmHWM` from `/proc/self/status`.
fn read_memory_status() -> anyhow::Result<MemoryStatus> {
    let text = std::fs::read_to_string("/proc/self/status").context("read /proc/self/status")?;
    parse_memory_status(&text).context("/proc/self/status has no VmRSS or no VmHWM line")
}

/// `VmRSS` and `VmHWM` from the text of `/proc/<pid>/status`, or `None` when either is missing
/// or not a number of kB.
pub(crate) fn parse_memory_status(text: &str) -> Option<MemoryStatus> {
    let field = |name: &str| {
        text.lines().find_map(|line| {
            let rest = line.strip_prefix(name)?.strip_prefix(':')?;
            rest.trim().strip_suffix("kB")?.trim().parse::<u64>().ok()
        })
    };
    Some(MemoryStatus {
        rss_kb: field("VmRSS")?,
        hwm_kb: field("VmHWM")?,
    })
}

#[cfg(test)]
#[path = "tests/txn_memory.rs"]
mod tests;
