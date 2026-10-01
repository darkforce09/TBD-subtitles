//! `report.md`: the quality check and the step timings, written for the owner to read.
//!
//! **Role:** render the summary, the findings per check and every flagged line with a timestamp,
//! then the step sections through `steps_table`: one row per step with its time, speed, CPU, GPU
//! and memory, the totals with the projection to a 120-minute video, and the phase times.
//!
//! **Position:** called by the job report with the check's result, the job record, the step
//! records and the last run.
//!
//! **Signals and state:** none; returns the text.
//!
//! **Invariants:** a measure that was not taken shows as `—`, never as 0; timestamps are
//! `h:mm:ss.d` of the video.

use std::fmt::Write;

use job_model::job::{JobRecord, JobRun, StepRecords};
use job_model::report::QcReport;

use super::{CPS_TARGET, steps_table};

/// The length the performance budget is written for.
pub const BUDGET_VIDEO_S: f64 = 7200.0;

/// The report as Markdown, with `steps` in its step table and `run`, the last run as a whole, in
/// its totals. `output` names the subtitle file, `dropped_sounds` the sound cues that found no
/// place.
pub fn render(
    report: &QcReport,
    record: &JobRecord,
    steps: &StepRecords,
    run: Option<&JobRun>,
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

    steps_table::steps_section(&mut md, steps, s.video_s, run);
    steps_table::phase_section(&mut md, steps, s.video_s);
    md
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
