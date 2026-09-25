//! `stack-spike report`: the recorded results as Markdown tables, and the 120-minute projection.

use crate::context::Context;
use crate::measure::{ItemResult, Status, read_results};

/// The length every result is projected to, in seconds.
const PROJECTED_S: f64 = 120.0 * 60.0;

pub(crate) fn run(ctx: &Context) -> anyhow::Result<()> {
    let results = read_results(ctx)?;
    println!(
        "| Item | Status | Wall s | Load s | Process s | × realtime | Peak VRAM MiB (process / device Δ) | Peak RAM MiB (worker / child) | 120-min projection, min |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for r in &results {
        println!(
            "| {} | {} | {:.1} | {:.1} | {:.1} | {:.1} | {} | {:.0} / {:.0} | {} |",
            r.item,
            status(r),
            r.wall_s,
            r.load_s,
            r.process_s,
            r.x_realtime,
            r.vram
                .map(|v| format!("{} / {}", v.process_mib, v.device_delta_mib))
                .unwrap_or_else(|| "not measured".into()),
            r.peak_ram_mib,
            r.peak_child_ram_mib,
            projection(r)
                .map(|m| format!("{m:.1}"))
                .unwrap_or_else(|| "—".into()),
        );
    }
    println!();
    for r in &results {
        if !r.notes.is_empty() {
            println!("{}: {}", r.item, serde_json::to_string(&r.notes)?);
        }
    }
    Ok(())
}

fn status(r: &ItemResult) -> String {
    match (&r.status, &r.reason) {
        (Status::Ok, _) => "ok".into(),
        (Status::Failed, Some(reason)) => format!("failed: {}", first_line(reason)),
        (Status::NotRun, Some(reason)) => format!("not run: {}", first_line(reason)),
        (s, None) => format!("{s:?}"),
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

/// Load time once, plus processing time scaled to 120 minutes of audio.
fn projection(r: &ItemResult) -> Option<f64> {
    (r.status == Status::Ok && r.audio_s > 0.0)
        .then(|| (r.load_s + r.process_s * PROJECTED_S / r.audio_s) / 60.0)
}
