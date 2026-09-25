//! The child side of a measurement: run one item, then report its times and peak RAM.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::context::Context;
use crate::items::Item;

/// What the worker writes to `results/<item>.worker.json` before it exits.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct WorkerReport {
    pub(crate) audio_s: f64,
    pub(crate) load_s: f64,
    pub(crate) process_s: f64,
    /// The process's resident-memory high-water mark (`VmHWM`).
    pub(crate) peak_ram_mib: f64,
    /// The largest peak resident memory of the programs the item ran, such as FFmpeg.
    pub(crate) peak_child_ram_mib: f64,
    pub(crate) notes: Map<String, Value>,
}

/// Run `item` in this process and write the report.
pub(crate) fn run(ctx: &Context, item: Item) -> anyhow::Result<()> {
    let outcome = item.run(ctx)?;
    let report = WorkerReport {
        audio_s: outcome.audio_s,
        load_s: outcome.load_s,
        process_s: outcome.process_s,
        peak_ram_mib: peak_ram_mib()?,
        peak_child_ram_mib: peak_child_ram_mib(),
        notes: outcome.notes,
    };
    ctx.write_json(&format!("results/{}.worker.json", item.name()), &report)
}

/// `ru_maxrss` of the children this process waited for, in MiB.
fn peak_child_ram_mib() -> f64 {
    // SAFETY: getrusage writes one plain struct that is zeroed first.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    let ok = unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, &mut usage) } == 0;
    if ok {
        usage.ru_maxrss as f64 / 1024.0
    } else {
        0.0
    }
}

/// `VmHWM` from `/proc/self/status`, in MiB.
fn peak_ram_mib() -> anyhow::Result<f64> {
    let status = std::fs::read_to_string("/proc/self/status")?;
    let kib = status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse::<f64>().ok())
        .ok_or_else(|| anyhow::anyhow!("no VmHWM line in /proc/self/status"))?;
    Ok(kib / 1024.0)
}
