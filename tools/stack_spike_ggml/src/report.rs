//! The worker report, in the shape `stack-spike` reads from `results/<item>.worker.json`.

use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};

/// What an item measured about itself.
#[derive(Debug, Default)]
pub(crate) struct Outcome {
    pub(crate) audio_s: f64,
    pub(crate) load_s: f64,
    pub(crate) process_s: f64,
    pub(crate) notes: Map<String, Value>,
}

impl Outcome {
    #[cfg_attr(
        not(feature = "crispasr"),
        expect(dead_code, reason = "only the ggml items write notes")
    )]
    pub(crate) fn note(&mut self, key: &str, value: impl Into<Value>) {
        self.notes.insert(key.to_string(), value.into());
    }
}

/// The same fields as `stack-spike`'s `WorkerReport`.
#[derive(Serialize)]
struct WorkerReport<'a> {
    audio_s: f64,
    load_s: f64,
    process_s: f64,
    peak_ram_mib: f64,
    peak_child_ram_mib: f64,
    notes: &'a Map<String, Value>,
}

/// Write the report, whole or not at all.
pub(crate) fn write(work: &Path, item: &str, outcome: Outcome) -> anyhow::Result<()> {
    let report = WorkerReport {
        audio_s: outcome.audio_s,
        load_s: outcome.load_s,
        process_s: outcome.process_s,
        peak_ram_mib: peak_ram_mib()?,
        peak_child_ram_mib: 0.0,
        notes: &outcome.notes,
    };
    let path = work.join("results").join(format!("{item}.worker.json"));
    let part = path.with_extension("json.part");
    std::fs::write(&part, serde_json::to_vec_pretty(&report)?)?;
    std::fs::rename(&part, &path)?;
    Ok(())
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
