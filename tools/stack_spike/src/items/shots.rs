//! The shots item: FFmpeg's scdet scan decoded on the CPU, with the NVDEC time as a note.

use std::time::{Duration, Instant};

use media_io::shot_changes;

use super::{Outcome, since};
use crate::context::Context;

const SCAN_DEADLINE: Duration = Duration::from_secs(3600);
/// The score thresholds whose cut counts are recorded.
const THRESHOLDS: [f64; 5] = [10.0, 15.0, 20.0, 25.0, 30.0];

pub(crate) fn run(ctx: &Context) -> anyhow::Result<Outcome> {
    let probe = ctx.probe()?;
    let start = Instant::now();
    let shots = shot_changes::scan(&ctx.programs, &ctx.video, false, SCAN_DEADLINE)?;
    let mut outcome = Outcome {
        audio_s: probe.duration_s,
        process_s: since(start),
        ..Outcome::default()
    };
    ctx.write_json("shots.json", &shots)?;
    for threshold in THRESHOLDS {
        let times = shots.times_at_least(threshold);
        let close = times.windows(2).filter(|w| w[1] - w[0] < 0.5).count();
        outcome.note(&format!("cuts_score_{threshold}"), times.len());
        outcome.note(&format!("cuts_score_{threshold}_within_half_s"), close);
    }
    let nvdec = Instant::now();
    let on_gpu = shot_changes::scan(&ctx.programs, &ctx.video, true, SCAN_DEADLINE)?;
    outcome.note("nvdec_scan_s", since(nvdec));
    outcome.note("nvdec_same_cuts", on_gpu.cuts.len() == shots.cuts.len());
    Ok(outcome)
}
