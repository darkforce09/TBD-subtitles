//! `report.md`: the quality check and the step timings, written for the owner to read.
//!
//! **Role:** render the summary, the findings per check, every flagged line with a timestamp,
//! and one row per step with its time and peak memory, plus the projection to a 120-minute video.
//!
//! **Position:** called by the job report with the check's result, the job record and the step
//! records.
//!
//! **Signals and state:** none; returns the text.
//!
//! **Invariants:** a measure that was not taken shows as `—`, never as 0; timestamps are
//! `h:mm:ss.d` of the video.

use std::fmt::Write;

use job_model::StepName;
use job_model::job::{JobRecord, StepRecords};
use job_model::report::QcReport;

use super::CPS_TARGET;

/// The length the performance budget is written for.
pub const BUDGET_VIDEO_S: f64 = 7200.0;

/// The report as Markdown, with `steps` in its step table. `output` names the subtitle file,
/// `dropped_sounds` the sound cues that found no place.
pub fn render(
    report: &QcReport,
    record: &JobRecord,
    steps: &StepRecords,
    output: &str,
    dropped_sounds: &[String],
) -> String {
    let s = &report.summary;
    let mut md = String::new();
    let video = std::path::Path::new(&record.video).file_name().map_or_else(
        || record.video.clone(),
        |n| n.to_string_lossy().into_owned(),
    );
    let _ = writeln!(md, "# Job report: {video}\n");
    let _ = writeln!(md, "- Video: `{}` ({})", record.video, clock(s.video_s));
    let _ = writeln!(md, "- Subtitles: `{output}`");
    let _ = writeln!(
        md,
        "- Cues: {} ({} dialogue, {} sound, {} music)",
        s.cues, s.dialogue_cues, s.sound_cues, s.music_cues
    );
    let verdict = if s.cps_ok_share >= CPS_TARGET {
        "meets"
    } else {
        "misses"
    };
    let _ = writeln!(
        md,
        "- Reading speed: {:.1} % of cues at or under 20 characters per second ({verdict} the {:.0} % target)",
        s.cps_ok_share * 100.0,
        CPS_TARGET * 100.0
    );
    let sources: Vec<String> = s
        .words_by_source
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    let _ = writeln!(md, "- Words by timing source: {}", sources.join(", "));
    let offset = s
        .offset_ms
        .map_or_else(|| "—".to_string(), |o| format!("{o:+.0} ms"));
    let _ = writeln!(
        md,
        "- Aligner offset from the backbone (signed median): {offset}"
    );
    let _ = writeln!(md, "- Unsure lines: {}; novel words: {}", s.unsure, s.novel);
    if s.reviewed > 0 {
        let _ = writeln!(md, "- Lines the owner corrected: {}", s.reviewed);
    }
    if s.fixed > 0 {
        let _ = writeln!(md, "- Lines Fix It changed, not checked yet: {}", s.fixed);
    }
    let _ = writeln!(
        md,
        "- Heard speech with no cue: {:.1} s; voice with no cue, words heard or not (grunts, crowds): {:.1} s",
        s.uncovered_speech_s, s.voice_without_cue_s
    );
    let layout = if report.has_layout_violations() {
        "**layout rules broken — see below**"
    } else {
        "no layout rule broken"
    };
    let _ = writeln!(md, "- Layout: {layout}\n");

    let _ = writeln!(md, "## Findings\n");
    let counts = report.counts();
    if counts.is_empty() {
        let _ = writeln!(md, "None.\n");
    } else {
        let _ = writeln!(md, "| Check | Count |\n|---|---|");
        for (check, count) in &counts {
            let _ = writeln!(md, "| {} | {count} |", check.describe());
        }
        md.push('\n');
    }

    let _ = writeln!(md, "## Flagged lines\n");
    if report.findings.is_empty() {
        let _ = writeln!(md, "None.\n");
    } else {
        let _ = writeln!(md, "| Time | Check | Text | Detail |\n|---|---|---|---|");
        for f in &report.findings {
            let _ = writeln!(
                md,
                "| {} | {} | {} | {} |",
                clock(f.time_s),
                f.check.describe(),
                cell(&f.text),
                cell(&f.detail)
            );
        }
        md.push('\n');
    }

    if !dropped_sounds.is_empty() {
        let _ = writeln!(md, "## Sound cues with no room\n");
        for d in dropped_sounds {
            let _ = writeln!(md, "- {}", cell(d));
        }
        md.push('\n');
    }

    let _ = writeln!(md, "## Steps\n");
    let _ = writeln!(
        md,
        "| Step | Stage | Wall s | Load s | Process s | Peak RAM MiB | Peak child RAM MiB | Peak VRAM MiB |\n|---|---|---|---|---|---|---|---|"
    );
    let mut total = 0.0;
    let mut peak_ram: Option<f64> = None;
    let mut peak_vram: Option<f64> = None;
    for step in StepName::ALL {
        let Some(r) = steps.get(&step) else {
            continue;
        };
        let m = &r.measure;
        total += m.wall_s;
        peak_ram = max(peak_ram, max(m.peak_ram_mib, m.peak_child_ram_mib));
        peak_vram = max(peak_vram, m.peak_vram_mib);
        let _ = writeln!(
            md,
            "| {step} | {} | {:.1} | {} | {} | {} | {} | {} |",
            step.stage(),
            m.wall_s,
            num(m.load_s, 1),
            num(m.process_s, 1),
            num(m.peak_ram_mib, 0),
            num(m.peak_child_ram_mib, 0),
            num(m.peak_vram_mib, 0)
        );
    }
    let scaled = if s.video_s > 0.0 {
        total * BUDGET_VIDEO_S / s.video_s / 60.0
    } else {
        0.0
    };
    let _ = writeln!(
        md,
        "\nTotal {:.1} min of step time for {}; scaled to a 120-minute video, {scaled:.1} min (budget 30 min). \
         The shot scan runs alongside separation and speech recognition, so wall time is lower. \
         Peak RAM {} MiB; peak VRAM {} MiB.",
        total / 60.0,
        clock(s.video_s),
        num(peak_ram, 0),
        num(peak_vram, 0)
    );
    md
}

fn max(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, None) => a,
        (None, b) => b,
    }
}

fn num(value: Option<f64>, decimals: usize) -> String {
    value.map_or_else(|| "—".to_string(), |v| format!("{v:.decimals$}"))
}

/// A table cell: no line breaks, no bare pipes.
fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

/// `h:mm:ss.d`.
pub fn clock(seconds: f64) -> String {
    let tenths = (seconds.max(0.0) * 10.0).round() as u64;
    format!(
        "{}:{:02}:{:02}.{}",
        tenths / 36_000,
        tenths / 600 % 60,
        tenths / 10 % 60,
        tenths % 10
    )
}

#[cfg(test)]
#[path = "tests/markdown.rs"]
mod tests;
