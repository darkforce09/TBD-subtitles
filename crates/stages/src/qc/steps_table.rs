//! The report's step sections: one row per step with its time, speed against the video, CPU,
//! GPU and memory, the job's totals, and the phase times of the steps that split theirs.
//!
//! **Role:** render `## Steps` with its footer (summed step time, the projection to a 120-minute
//! video, the run's real wall time and the whole job's peak memory) and `## Phase times`.
//!
//! **Position:** called by `markdown::render` with the step records, the video's length and the
//! last run.
//!
//! **Signals and state:** none; appends to the text it is given.
//!
//! **Invariants:** a measure that was not taken shows as `—`, never as 0; speeds and frame rates
//! are computed here from the stored times and notes, never stored; a phase line appears only for
//! a step whose record holds that phase.

use std::fmt::Write;

use job_model::StepName;
use job_model::job::{JobRun, StepMeasure, StepRecords};

use super::markdown::{BUDGET_VIDEO_S, clock};

/// The shortest wall time a speed against the video is shown for, in seconds.
const SHORTEST_TIMED_S: f64 = 0.1;

/// Append `## Steps`: every step's row, then the totals. `video_s` is the video's length.
pub(super) fn steps_section(
    md: &mut String,
    steps: &StepRecords,
    video_s: f64,
    run: Option<&JobRun>,
) {
    let _ = writeln!(md, "## Steps\n");
    let _ = writeln!(
        md,
        "| Step | Stage | Wall s | × RT | Load s | Process s | CPU cores mean/peak | Hot thread % | GPU busy % | Peak RAM MiB | Peak child RAM MiB | Job RAM MiB | Peak VRAM MiB |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|"
    );
    let mut total = 0.0;
    let mut peak_ram: Option<f64> = None;
    let mut peak_vram: Option<f64> = None;
    let mut job_ram: Option<f64> = None;
    for step in StepName::ALL {
        let Some(r) = steps.get(&step) else {
            continue;
        };
        let m = &r.measure;
        total += m.wall_s;
        peak_ram = max(peak_ram, max(m.peak_ram_mib, m.peak_child_ram_mib));
        peak_vram = max(peak_vram, m.peak_vram_mib);
        job_ram = max(job_ram, m.job_ram_mib);
        let _ = writeln!(
            md,
            "| {step} | {} | {:.1} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            step.stage(),
            m.wall_s,
            num(speed(video_s, m.wall_s), 1),
            num(m.load_s, 1),
            num(m.process_s, 1),
            cores(m),
            num(m.busiest_thread_pct, 0),
            num(m.gpu_busy_pct, 0),
            num(m.peak_ram_mib, 0),
            num(m.peak_child_ram_mib, 0),
            num(m.job_ram_mib, 0),
            num(m.peak_vram_mib, 0)
        );
    }
    let scaled = if video_s > 0.0 {
        total * BUDGET_VIDEO_S / video_s / 60.0
    } else {
        0.0
    };
    let _ = writeln!(
        md,
        "\nTotal {:.1} min of step time for {}; scaled to a 120-minute video, {scaled:.1} min (budget 30 min). \
         The shot scan runs alongside separation and speech recognition, so wall time is lower. \
         Peak RAM {} MiB; peak VRAM {} MiB.",
        total / 60.0,
        clock(video_s),
        num(peak_ram, 0),
        num(peak_vram, 0)
    );
    let _ = writeln!(
        md,
        "\nReal wall time of the last run {} min against {:.1} min of summed step time, which counts every stored step, \
         also those an earlier run finished and this one skipped. \
         Whole-job peak RAM, every process of the job at once: {} MiB across the steps, {} MiB over the last run.",
        num(run.map(|run| run.wall_s() / 60.0), 1),
        total / 60.0,
        num(job_ram, 0),
        num(run.and_then(|run| run.peak_ram_mib), 0)
    );
}

/// Append `## Phase times` for the steps whose records split their time or count their frames;
/// nothing when none does.
pub(super) fn phase_section(md: &mut String, steps: &StepRecords, video_s: f64) {
    let lines: Vec<String> = [
        detect_line(steps),
        localized_line(steps),
        shot_scan_line(steps, video_s),
        verify_line(steps),
    ]
    .into_iter()
    .flatten()
    .collect();
    if lines.is_empty() {
        return;
    }
    let _ = writeln!(md, "\n## Phase times\n");
    for line in lines {
        let _ = writeln!(md, "- {line}");
    }
}

/// The text detection's decode wait, detection, confirmation and stills, its frames and NVDEC use.
fn detect_line(steps: &StepRecords) -> Option<String> {
    let m = &steps.get(&StepName::TextDetect)?.measure;
    let mut parts = phases(
        m,
        &[
            ("decode wait", "decode_wait_s"),
            ("detection", "detect_s"),
            ("confirmation", "confirm_s"),
            ("stills", "stills_s"),
        ],
    );
    for (label, key) in [
        ("decoded", "frames_decoded"),
        ("screened", "frames_screened"),
    ] {
        if let Some(frames) = note(m, key) {
            parts.push(format!(
                "{frames:.0} frames {label} ({} fps)",
                num(rate(frames, m.process_s.unwrap_or(0.0)), 0)
            ));
        }
    }
    if let Some(decoder) = m.gpu_decoder_pct.filter(|pct| *pct > 0.0) {
        parts.push(format!("NVDEC {decoder:.0} % mean"));
    }
    line(StepName::TextDetect, parts)
}

/// The localized video's decode wait, blend, encode wait and flush, its frames and NVENC use.
fn localized_line(steps: &StepRecords) -> Option<String> {
    let m = &steps.get(&StepName::LocalizedVideo)?.measure;
    let mut parts = phases(
        m,
        &[
            ("decode wait", "decode_wait_s"),
            ("blend", "blend_s"),
            ("encode wait", "encode_wait_s"),
            ("flush", "flush_s"),
        ],
    );
    if let Some(frames) = note(m, "frames") {
        parts.push(format!(
            "{frames:.0} frames ({} fps)",
            num(rate(frames, m.process_s.unwrap_or(0.0)), 1)
        ));
    }
    if let Some(encoder) = m.gpu_encoder_pct {
        parts.push(format!("NVENC {encoder:.0} % mean"));
    }
    line(StepName::LocalizedVideo, parts)
}

/// The shot scan's frame rate: the video's frames, from its length and the probe's rate, over the
/// scan's wall time.
fn shot_scan_line(steps: &StepRecords, video_s: f64) -> Option<String> {
    let m = &steps.get(&StepName::ShotScan)?.measure;
    let fps = note(&steps.get(&StepName::ProbeDecode)?.measure, "fps")?;
    let frames = video_s * fps;
    let speed = rate(frames, m.wall_s)?;
    line(
        StepName::ShotScan,
        vec![format!("{frames:.0} frames at {speed:.0} fps")],
    )
}

/// The read-back check's samples per second.
fn verify_line(steps: &StepRecords) -> Option<String> {
    let m = &steps.get(&StepName::TextVerify)?.measure;
    let samples = note(m, "samples")?;
    let speed = rate(samples, m.process_s?)?;
    line(
        StepName::TextVerify,
        vec![format!("{samples:.0} samples at {speed:.1} per second")],
    )
}

/// Each phase the record notes, in seconds with its share of the processing time.
fn phases(m: &StepMeasure, phases: &[(&str, &str)]) -> Vec<String> {
    phases
        .iter()
        .filter_map(|(label, key)| {
            let seconds = note(m, key)?;
            let share = m
                .process_s
                .filter(|total| *total > 0.0)
                .map_or_else(String::new, |total| {
                    format!(" ({:.0} %)", seconds / total * 100.0)
                });
            Some(format!("{label} {seconds:.1} s{share}"))
        })
        .collect()
}

fn line(step: StepName, parts: Vec<String>) -> Option<String> {
    (!parts.is_empty()).then(|| format!("{step}: {}", parts.join("; ")))
}

/// A note that reads as a number.
fn note(m: &StepMeasure, key: &str) -> Option<f64> {
    m.notes.get(key)?.parse().ok()
}

/// `amount` per second of `seconds`; `None` without time or amount.
fn rate(amount: f64, seconds: f64) -> Option<f64> {
    (amount > 0.0 && seconds > 0.0).then(|| amount / seconds)
}

/// The video's length over a step's wall time; `None` for a step shorter than `SHORTEST_TIMED_S`,
/// whose time is too small to divide by.
fn speed(video_s: f64, wall_s: f64) -> Option<f64> {
    rate(video_s, wall_s).filter(|_| wall_s >= SHORTEST_TIMED_S)
}

/// The mean and peak cores busy, `mean / peak`; a dash when neither was measured.
fn cores(m: &StepMeasure) -> String {
    if m.cpu_cores_mean.is_none() && m.cpu_cores_peak.is_none() {
        return "—".to_string();
    }
    format!(
        "{} / {}",
        num(m.cpu_cores_mean, 1),
        num(m.cpu_cores_peak, 1)
    )
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

#[cfg(test)]
#[path = "tests/steps_table.rs"]
mod tests;
