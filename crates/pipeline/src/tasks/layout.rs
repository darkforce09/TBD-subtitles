//! The layout tasks: cue building, the quality check, and the subtitle file beside the video.
//!
//! **Role:** build the cue track at the video's frame rate, check it and everything flagged
//! before it, and install the subtitle file in the job's output format.
//!
//! **Position:** called by `tasks::run` inside the job runner; calls `stages::{cues, qc, output}`
//! and the subtitle writers.
//!
//! **Signals and state:** reads the outputs of the earlier steps (the words from `reviewed.json`;
//! for the check, the corrections, and the re-decodes so Fix It's words are held against every
//! hypothesis); writes `cues.json`, `cues_dropped_sounds.json`,
//! `qc.json`, `output.json` and the subtitle file beside the video.
//!
//! **Invariants:** the frame rate comes from the probe (24/1 when the video has none); the subtitle
//! file is the only file written outside the work directory; a Fix It change the owner has not
//! checked has its words checked again, and is counted apart from the owner's corrections.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use job_model::StepName;
use job_model::job::OutputFormat;
use job_model::onscreen::TextDocument;
use job_model::outputs::{
    AdjudicationPass, Aligned, Corrections, EngineTranscript, Line, OutputRecord, Redecode,
    ShotChanges, SoundCues, SpeechPlan, TimeSpan, Utterance,
};
use stages::adjudication::{checks, redecode};
use stages::{cues, output, qc};
use subtitle_formats::cue::{CueTrack, FrameRate};
use subtitle_formats::writers::{ass, srt, vtt};

use super::{Job, TaskReport, since};
use crate::error::{Context, Result};
use crate::work_dir;

pub(super) fn cues(job: &Job) -> Result<TaskReport> {
    let probe = job.probe()?;
    let aligned: Aligned = work_dir::read_json(&job.work.reviewed())?;
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
    let aligned: Aligned = work_dir::read_json(&job.work.reviewed())?;
    let corrections: Corrections = if job.work.review().exists() {
        work_dir::read_json(&job.work.review())?
    } else {
        Corrections::default()
    };
    let sheet: Vec<Utterance> = work_dir::read_json(&job.work.sheet())?;
    let redecoded = |engine: &str| work_dir::read_json::<Redecode>(&job.work.redecode(engine)).ok();
    let (again_p, again_w) = (redecoded("parakeet"), redecoded("whisper"));
    let alternatives: Vec<(&str, &Redecode)> = [("p", &again_p), ("w", &again_w)]
        .into_iter()
        .filter_map(|(tag, r)| r.as_ref().map(|r| (tag, r)))
        .collect();
    let adjudicated = settled(
        work_dir::read_json::<AdjudicationPass>(&job.work.adjudicated())?,
        &corrections,
        &redecode::with_alternatives(&sheet, &alternatives),
        &job.glossary(),
    );
    let sound_cues: SoundCues = work_dir::read_json(&job.work.sound_cues())?;
    let speech: SpeechPlan = work_dir::read_json(&job.work.vad())?;
    let parakeet: EngineTranscript = work_dir::read_json(&job.work.asr("parakeet"))?;
    // The backbone times words closely; Whisper stretches and shifts them, and hears laughs the
    // language model rightly drops.
    let heard = qc::coverage::heard_spans(&[&parakeet]);
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
    let mut result = qc::check(&qc::QcInput {
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
    result.summary.reviewed = corrections.owner_count();
    result.summary.fixed = corrections.unchecked_fix_count();
    let mut report = TaskReport {
        process_s: since(started),
        ..TaskReport::default()
    };
    if job.settings().onscreen_text.enabled {
        note_text_quality(job, &mut report)?;
    }
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

/// Visual warnings keep their occurrence IDs and do not become dialogue findings.
fn note_text_quality(job: &Job, report: &mut TaskReport) -> Result<()> {
    let text: TextDocument = work_dir::read_json(&job.work.text(StepName::TextTypeset))?;
    let summary = text.summary();
    report.note("text_detected", summary.detected);
    report.note("text_translated", summary.translated);
    report.note("text_fallback", summary.fallback);
    report.note("text_unresolved", summary.unresolved);
    report.note("text_flagged", summary.flagged);
    for warning in &text.review_warnings {
        tracing::warn!("{warning}");
    }
    for occurrence in text.occurrences.iter().filter(|text| !text.reviewed) {
        if occurrence.english.is_none() {
            tracing::warn!(
                occurrence = %occurrence.id,
                time_s = occurrence.start_s,
                "on-screen writing needs a verified translation"
            );
        }
        for warning in &occurrence.warnings {
            tracing::warn!(occurrence = %occurrence.id, time_s = occurrence.start_s, "{warning}");
        }
    }
    Ok(())
}

/// The adjudication as the corrections left it: each corrected line's text and flags; no novel or
/// dropped-word finding on a corrected line, since the owner settled it or its Fix It change is
/// listed for the owner; and the words of each Fix It change the owner has not checked held
/// again against what the engines heard (`sheet`, with the re-decoded alternatives).
fn settled(
    mut pass: AdjudicationPass,
    corrections: &Corrections,
    sheet: &[Utterance],
    glossary: &[&str],
) -> AdjudicationPass {
    pass.lines = super::review::corrected_lines(&pass.lines, corrections);
    let open = |(id, _): &(String, String)| corrections.get(id).is_none();
    pass.findings.novel.retain(open);
    pass.findings.removed_locked.retain(open);
    let unchecked: Vec<Line> = pass
        .lines
        .iter()
        .filter(|l| {
            corrections
                .get(&l.id)
                .is_some_and(|c| c.chosen.is_unchecked_fix())
        })
        .cloned()
        .collect();
    if !unchecked.is_empty() {
        let again = checks::check(sheet, &unchecked, glossary);
        pass.findings.novel.extend(again.novel);
    }
    pass
}

pub(super) fn output(job: &Job) -> Result<TaskReport> {
    let track: CueTrack = work_dir::read_json(&job.work.cues())?;
    let started = Instant::now();
    let format = job.settings().effective_output_format();
    let mut text = match format {
        OutputFormat::Srt => srt::write(&track),
        OutputFormat::Vtt => vtt::write(&track),
        OutputFormat::Ass => ass::write(&track),
    };
    if job.settings().onscreen_text.enabled {
        let events = std::fs::read_to_string(job.work.text_ass())
            .context("cannot read typeset on-screen text events")?;
        text.push_str(&events);
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        .to_string();
    let earlier = work_dir::read_json::<OutputRecord>(&job.work.output_record())
        .ok()
        .map(|r| PathBuf::from(r.path));
    let installed = output::install(
        &job.video(),
        format,
        &text,
        &job.work.backup(),
        &stamp,
        earlier.as_deref(),
    )
    .context(format!(
        "cannot write the subtitle file beside {}",
        job.record.video
    ))?;
    let mut report = TaskReport {
        process_s: since(started),
        ..TaskReport::default()
    };
    report.note("path", installed.path.display());
    report.note("unchanged", installed.unchanged);
    let shown = |p: &Option<PathBuf>| p.as_ref().map(|p| p.to_string_lossy().into_owned());
    if let Some(backup) = shown(&installed.backup) {
        report.note("backup", &backup);
    }
    if let Some(retired) = shown(&installed.retired) {
        report.note("retired", &retired);
    }
    let localized = if super::replace::localized(job) {
        let events = std::fs::read_to_string(job.work.text_ass_localized())
            .context("cannot read the localized video's on-screen text events")?;
        let mut localized_text = ass::write(&track);
        localized_text.push_str(&events);
        let installed = output::install_localized_subtitles(
            &job.video(),
            &localized_text,
            &job.work.backup(),
            &stamp,
        )
        .context(format!(
            "cannot write the localized subtitle file beside {}",
            job.record.video
        ))?;
        report.note("localized", installed.path.display());
        if let Some(backup) = shown(&installed.backup) {
            report.note("localized_backup", &backup);
        }
        shown(&Some(installed.path))
    } else {
        None
    };
    let record = OutputRecord {
        path: installed.path.to_string_lossy().into_owned(),
        unchanged: installed.unchanged,
        backup: shown(&installed.backup),
        retired: shown(&installed.retired),
        localized,
    };
    work_dir::write_json(&job.work.output_record(), &record)?;
    Ok(report)
}

#[cfg(test)]
#[path = "tests/layout.rs"]
mod tests;
