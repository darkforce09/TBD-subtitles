//! The layout tasks: cue building, the quality check, and the subtitle file beside the video.
//!
//! **Role:** build the cue track at the video's frame rate, check it and everything flagged
//! before it, and install the SRT file.
//!
//! **Position:** called by `tasks::run` inside the job runner; calls `stages::{cues, qc, output}`
//! and the SRT writer.
//!
//! **Signals and state:** reads the outputs of the earlier steps; writes `cues.json`,
//! `cues_dropped_sounds.json`, `qc.json`, `output.json` and the subtitle file beside the video.
//!
//! **Invariants:** the frame rate comes from the probe (24/1 when the video has none); the subtitle
//! file is the only file written outside the work directory.

use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use job_model::outputs::{
    AdjudicationPass, Aligned, EngineTranscript, ShotChanges, SoundCues, SpeechPlan, TimeSpan,
    Utterance,
};
use stages::{cues, output, qc};
use subtitle_formats::cue::{CueTrack, FrameRate};
use subtitle_formats::writers::srt;

use super::{Job, TaskReport, since};
use crate::error::{Context, Result};
use crate::work_dir;

pub(super) fn cues(job: &Job) -> Result<TaskReport> {
    let probe = job.probe()?;
    let aligned: Aligned = work_dir::read_json(&job.work.aligned())?;
    let sounds: SoundCues = work_dir::read_json(&job.work.sound_cues())?;
    let shots: ShotChanges = work_dir::read_json(&job.work.shots())?;
    let rate = probe
        .probe
        .video
        .as_ref()
        .and_then(|v| FrameRate::new(v.frame_rate_num, v.frame_rate_den))
        .unwrap_or(FrameRate::FILM);
    let started = Instant::now();
    let built = cues::build(
        &aligned,
        &sounds.cues,
        &shots,
        job.settings().cut_score,
        rate,
        probe.probe.duration_s,
    );
    let mut report = TaskReport {
        process_s: since(started),
        ..TaskReport::default()
    };
    report.note("cues", built.track.cues.len());
    report.note("dropped_sounds", built.dropped_sounds.len());
    report.note("frame_rate", format!("{}/{}", rate.num(), rate.den()));
    work_dir::write_json(&job.work.cues(), &built.track)?;
    work_dir::write_json(&job.work.dropped_sounds(), &built.dropped_sounds)?;
    Ok(report)
}

pub(super) fn qc(job: &Job) -> Result<TaskReport> {
    let probe = job.probe()?;
    let track: CueTrack = work_dir::read_json(&job.work.cues())?;
    let aligned: Aligned = work_dir::read_json(&job.work.aligned())?;
    let adjudicated: AdjudicationPass = work_dir::read_json(&job.work.adjudicated())?;
    let sound_cues: SoundCues = work_dir::read_json(&job.work.sound_cues())?;
    let speech: SpeechPlan = work_dir::read_json(&job.work.vad())?;
    let parakeet: EngineTranscript = work_dir::read_json(&job.work.asr("parakeet"))?;
    // The backbone times words closely; Whisper stretches and shifts them, and hears laughs the
    // language model rightly drops.
    let heard = qc::coverage::heard_spans(&[&parakeet]);
    let sheet: Vec<Utterance> = work_dir::read_json(&job.work.sheet())?;
    let started = Instant::now();
    let dropped_ids: Vec<&str> = adjudicated
        .lines
        .iter()
        .filter(|l| l.has_flag("DROP"))
        .map(|l| l.id.as_str())
        .collect();
    let dropped: Vec<TimeSpan> = sheet
        .iter()
        .filter(|u| dropped_ids.contains(&u.id.as_str()))
        .map(|u| TimeSpan::new(u.start_s, u.end_s))
        .collect();
    let starts: HashMap<String, f64> = sheet.iter().map(|u| (u.id.clone(), u.start_s)).collect();
    let result = qc::check(&qc::QcInput {
        track: &track,
        aligned: &aligned,
        adjudicated: &adjudicated,
        sound_cues: &sound_cues,
        speech: &speech,
        heard: &heard,
        dropped: &dropped,
        utterance_starts: &starts,
        duration_s: probe.probe.duration_s,
    });
    let mut report = TaskReport {
        process_s: since(started),
        ..TaskReport::default()
    };
    report.note("findings", result.findings.len());
    report.note(
        "layout_violations",
        result
            .findings
            .iter()
            .filter(|f| f.check.is_layout_violation())
            .count(),
    );
    report.note(
        "cps_ok_share",
        format!("{:.3}", result.summary.cps_ok_share),
    );
    work_dir::write_json(&job.work.qc(), &result)?;
    Ok(report)
}

pub(super) fn output(job: &Job) -> Result<TaskReport> {
    let track: CueTrack = work_dir::read_json(&job.work.cues())?;
    let started = Instant::now();
    let text = srt::write(&track);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        .to_string();
    let installed = output::install(&job.video(), &text, &job.work.backup(), &stamp).context(
        format!("cannot write the subtitle file beside {}", job.record.video),
    )?;
    let mut report = TaskReport {
        process_s: since(started),
        ..TaskReport::default()
    };
    report.note("path", installed.path.display());
    report.note("unchanged", installed.unchanged);
    if let Some(backup) = &installed.backup {
        report.note("backup", backup.display());
    }
    work_dir::write_json(&job.work.output_record(), &report.notes)?;
    Ok(report)
}
