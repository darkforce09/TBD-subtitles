//! One row of a detector-pool table: what a configuration measured, or why it failed.
//!
//! **Role:** hold what one pool run measured (frames, wall time and GPU use, peak VRAM, warm-up
//! and engine build seconds, the pool's notes and the regions found) and lay it out as table
//! cells, reading from the notes whether every session kept the CUDA graph and NHWC and whether
//! each TensorRT engine was built or reused.
//! **Position:** used by the pool sections of `detect-bench`; the measuring itself is in
//! `mod.rs`.
//! **Signals and state:** none; plain values.
//! **Invariants:** a failed configuration gives a row with its configuration and the error in
//! the last cell, never a missing row; a value that was not measured reads `n/a`.

use std::collections::BTreeMap;

use pipeline::measure::gpu_monitor::VramPeaks;

use super::super::table::optional;
use super::super::usage::Usage;
use super::compare::Regions;
use super::plan::PoolRun;

/// Which sessions a row measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Screen,
    Confirm,
}

impl Phase {
    /// The prefix of the pool's notes about this phase's sessions.
    fn notes_prefix(self) -> &'static str {
        match self {
            Phase::Screen => "screen",
            Phase::Confirm => "confirm",
        }
    }
}

/// What one pool configuration measured.
#[derive(Debug, Clone)]
pub struct Measured {
    /// Frames in the timed part.
    pub frames: usize,
    pub usage: Usage,
    pub vram: Option<VramPeaks>,
    pub warmup_s: f64,
    pub engine_build_s: f64,
    pub notes: BTreeMap<String, String>,
    /// Every frame's regions, in frame order.
    pub regions: Regions,
}

impl Measured {
    pub fn fps(&self) -> f64 {
        self.frames as f64 / self.usage.wall_s
    }

    pub fn boxes(&self) -> usize {
        self.regions.iter().map(Vec::len).sum()
    }
}

/// The pool tables' columns.
pub const COLUMNS: [&str; 18] = [
    "Engine",
    "Search",
    "Sessions",
    "Batch",
    "Pool MiB",
    "Frames",
    "Frames/s",
    "ms/frame",
    "GPU %",
    "VRAM process MiB",
    "VRAM device Δ MiB",
    "Warm-up s",
    "Engine build s",
    "Engine cache",
    "CUDA graph + NHWC",
    "Boxes",
    "Against reference",
    "Note",
];

/// The cells of `run`'s row in `phase`, with `against` describing its boxes against a
/// reference run when there is one.
pub fn row(
    run: &PoolRun,
    phase: Phase,
    outcome: &Result<Measured, String>,
    against: Option<&str>,
) -> Vec<String> {
    let (batch, pool_mib) = match phase {
        Phase::Screen => (run.shape.batch, run.shape.pool_mib),
        Phase::Confirm => (1, inference::ocr::pool::CONFIRM_POOL_MIB),
    };
    let mut cells = vec![
        run.engine_label().to_string(),
        run.search_label().to_string(),
        run.sessions.to_string(),
        batch.to_string(),
        pool_mib.to_string(),
    ];
    match outcome {
        Ok(m) => cells.extend([
            m.frames.to_string(),
            format!("{:.1}", m.fps()),
            format!("{:.2}", m.usage.wall_s * 1000.0 / m.frames.max(1) as f64),
            optional(m.usage.gpu_pct, 0),
            optional(m.vram.map(|v| v.process_mib as f64), 0),
            optional(m.vram.map(|v| v.device_delta_mib as f64), 0),
            format!("{:.1}", m.warmup_s),
            format!("{:.1}", m.engine_build_s),
            engine_cache(&m.notes, phase),
            tuning(&m.notes, phase),
            m.boxes().to_string(),
            against.unwrap_or("—").to_string(),
            String::new(),
        ]),
        Err(error) => {
            cells.extend(std::iter::repeat_n(String::new(), 12));
            cells.push(format!("error: {error}"));
        }
    }
    cells
}

/// Whether `phase`'s sessions kept the CUDA graph and NHWC: `accepted ×n`, or how many refused
/// and the first refusal; `n/a` when no session of the phase opened.
pub fn tuning(notes: &BTreeMap<String, String>, phase: Phase) -> String {
    const ON: &str = "CUDA graph and NHWC on";
    const REFUSED: &str = "CUDA graph and NHWC off; the session refused them: ";
    let key = format!("{} session options", phase.notes_prefix());
    let values: Vec<&str> = notes
        .iter()
        .filter(|(name, _)| name.starts_with(&key))
        .map(|(_, value)| value.as_str())
        .collect();
    let refused: Vec<&str> = values.iter().copied().filter(|v| *v != ON).collect();
    match (values.len(), refused.first()) {
        (0, _) => "n/a".to_string(),
        (all, None) => format!("accepted ×{all}"),
        (all, Some(first)) => format!(
            "refused ×{} of {all}: {}",
            refused.len(),
            first.strip_prefix(REFUSED).unwrap_or(first)
        ),
    }
}

/// Whether `phase`'s TensorRT engines were built on this run or reused from the cache; `—` for
/// sessions without TensorRT.
pub fn engine_cache(notes: &BTreeMap<String, String>, phase: Phase) -> String {
    let key = format!("{} engine cache", phase.notes_prefix());
    let (mut built, mut reused) = (0, 0);
    for (_, value) in notes.iter().filter(|(name, _)| name.starts_with(&key)) {
        if value.starts_with("built") {
            built += 1;
        } else {
            reused += 1;
        }
    }
    match (built, reused) {
        (0, 0) => "—".to_string(),
        (built, 0) => format!("built ×{built}"),
        (0, reused) => format!("reused ×{reused}"),
        (built, reused) => format!("built ×{built}, reused ×{reused}"),
    }
}

#[cfg(test)]
#[path = "tests/rows.rs"]
mod tests;
