//! The processes of a job read from `/proc`: which pids descend from the app, how much memory
//! they hold together, and how many CPU ticks each of them and each of their threads used.
//!
//! **Role:** parse `/proc/<pid>/stat` and `/proc/<pid>/smaps_rollup`, walk every pid's parent to
//! find a root's descendants, and read their summed proportional set size and their per-process
//! and per-thread CPU ticks.
//!
//! **Position:** read by `job_sampler` four times a second; the parsers are pure, so the tests
//! feed them fixture text.
//!
//! **Signals and state:** none; every call reads `/proc` afresh.
//!
//! **Invariants:** a pid or thread that vanishes between the listing and the read is skipped
//! silently, never an error; a process name holding spaces or parentheses parses, since the
//! fields after the name are split from its last `)`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

/// The fields of one `stat` line the sampler reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stat {
    /// The parent's pid (field 4).
    pub ppid: u32,
    /// User-mode ticks (field 14).
    pub utime: u64,
    /// Kernel-mode ticks (field 15).
    pub stime: u64,
}

impl Stat {
    /// User and kernel ticks together.
    pub fn ticks(&self) -> u64 {
        self.utime + self.stime
    }
}

/// The ticks per second of `utime` and `stime`: `sysconf(_SC_CLK_TCK)`, or 100 when it reports
/// none.
pub fn clock_ticks() -> f64 {
    // SAFETY: sysconf reads one configuration value and has no other effect.
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if ticks > 0 { ticks as f64 } else { 100.0 }
}

/// One `stat` line's fields; `None` when it does not parse.
pub fn parse_stat(line: &str) -> Option<Stat> {
    let (_, rest) = line.rsplit_once(')')?;
    // `rest` starts at field 3 (the state), so field n is at index n - 3.
    let fields: Vec<&str> = rest.split_whitespace().collect();
    Some(Stat {
        ppid: fields.get(1)?.parse().ok()?,
        utime: fields.get(11)?.parse().ok()?,
        stime: fields.get(12)?.parse().ok()?,
    })
}

/// The `Pss:` line of an `smaps_rollup` file, in KiB; `None` when it has none.
pub fn parse_pss_kib(rollup: &str) -> Option<u64> {
    rollup
        .lines()
        .find_map(|line| line.strip_prefix("Pss:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse().ok())
}

/// `root` and every pid that descends from it in `parents` (each pid's parent).
pub fn descendants(root: u32, parents: &BTreeMap<u32, u32>) -> BTreeSet<u32> {
    let mut tree = BTreeSet::from([root]);
    let mut grew = true;
    while grew {
        grew = false;
        for (&pid, parent) in parents {
            if tree.contains(parent) && tree.insert(pid) {
                grew = true;
            }
        }
    }
    tree
}

/// Every pid's stat line, by pid; pids that vanish while read are left out.
pub fn all_stats() -> BTreeMap<u32, Stat> {
    let Ok(entries) = fs::read_dir("/proc") else {
        return BTreeMap::new();
    };
    entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter_map(|pid| Some((pid, read_stat(&format!("/proc/{pid}/stat"))?)))
        .collect()
}

/// The stat of each process of `root`'s tree, by pid.
pub fn tree_stats(root: u32) -> BTreeMap<u32, Stat> {
    let stats = all_stats();
    let parents = stats.iter().map(|(&pid, stat)| (pid, stat.ppid)).collect();
    let tree = descendants(root, &parents);
    stats
        .into_iter()
        .filter(|(pid, _)| tree.contains(pid))
        .collect()
}

/// The summed proportional set size of `pids`, in MiB; `None` when none could be read.
pub fn pss_mib(pids: impl IntoIterator<Item = u32>) -> Option<f64> {
    let kib: Vec<u64> = pids
        .into_iter()
        .filter_map(|pid| {
            let rollup = fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).ok()?;
            parse_pss_kib(&rollup)
        })
        .collect();
    (!kib.is_empty()).then(|| kib.iter().sum::<u64>() as f64 / 1024.0)
}

/// The ticks of every thread of `pids`, by thread id.
pub fn thread_ticks(pids: impl IntoIterator<Item = u32>) -> BTreeMap<u32, u64> {
    let mut ticks = BTreeMap::new();
    for pid in pids {
        let Ok(threads) = fs::read_dir(format!("/proc/{pid}/task")) else {
            continue;
        };
        for tid in threads
            .flatten()
            .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        {
            if let Some(stat) = read_stat(&format!("/proc/{pid}/task/{tid}/stat")) {
                ticks.insert(tid, stat.ticks());
            }
        }
    }
    ticks
}

fn read_stat(path: &str) -> Option<Stat> {
    parse_stat(&fs::read_to_string(path).ok()?)
}

#[cfg(test)]
#[path = "tests/process_tree.rs"]
mod tests;
