//! The speech tasks: voice activity and the chunk plan, both engines over the chunks, the diff
//! sheet, and both engines again over the unsure utterances.
//!
//! **Role:** plan the chunks on the vocal stem; run Parakeet (ONNX) and Whisper (ggml) over the
//! mix; line them up into the sheet; hear the unsure utterances again on the vocal stem.
//!
//! **Position:** called by `tasks::run`: Parakeet in the main binary, Whisper in the ggml binary
//! (the `crispasr` feature), the plan and the sheet in the job runner.
//!
//! **Signals and state:** reads the audio files, `vad.json`, `sheet.json` and
//! `adjudication/first.json`; writes `vad.json`, `asr/<engine>.json`, `sheet.json`, `sheet.txt`
//! and `adjudication/redecode_<engine>.json`.
//!
//! **Invariants:** every engine hears the same chunk plan; the re-decode loads no model when
//! nothing is unsure; a binary without CrispASR refuses Whisper instead of skipping it.

use std::time::Instant;

use inference::onnx::Device;
use inference::onnx::parakeet_tdt::{self, ParakeetTdt};
use job_model::job::WhisperModel;
use job_model::outputs::{AdjudicationPass, EngineTranscript, Redecode, SpeechPlan};
use stages::adjudication::redecode;
use stages::asr::{self, SpeechEngine};
use stages::diff_sheet::sheet;
use stages::vad::{self, VadSettings};

use super::{Job, TaskReport, since};
use crate::error::{Context, PipelineError, Result};
use crate::work_dir;

/// Threads CrispASR runs Whisper's CPU parts on.
#[cfg_attr(not(feature = "crispasr"), allow(dead_code))]
const WHISPER_THREADS: i32 = 8;

pub(super) fn vad(job: &Job) -> Result<TaskReport> {
    let duration = job.probe()?.probe.duration_s;
    let started = Instant::now();
    let scores = vad::score_file(&job.work.vocals()).context("score the vocal stem")?;
    let plan = vad::plan(&scores, duration, &VadSettings::default());
    let mut report = TaskReport {
        process_s: since(started),
        ..TaskReport::default()
    };
    report.note("regions", plan.regions.len());
    report.note("chunks", plan.chunks.len());
    report.note("speech_s", format!("{:.1}", plan.speech_s()));
    work_dir::write_json(&job.work.vad(), &plan)?;
    Ok(report)
}

pub(super) fn asr_parakeet(job: &Job, progress: &dyn Fn(usize, usize)) -> Result<TaskReport> {
    let plan: SpeechPlan = work_dir::read_json(&job.work.vad())?;
    let load = Instant::now();
    let mut engine = parakeet(job)?;
    let mut report = TaskReport {
        load_s: since(load),
        ..TaskReport::default()
    };
    let started = Instant::now();
    let transcript = asr::transcribe_plan(&mut engine, &job.work.mix(), "mix", &plan, progress)
        .map_err(|e| PipelineError::new("Parakeet", e))?;
    report.process_s = since(started);
    report.note("words", transcript.words().count());
    work_dir::write_json(&job.work.asr("parakeet"), &transcript)?;
    Ok(report)
}

pub(super) fn asr_whisper(job: &Job, progress: &dyn Fn(usize, usize)) -> Result<TaskReport> {
    let plan: SpeechPlan = work_dir::read_json(&job.work.vad())?;
    let load = Instant::now();
    let mut engine = whisper(job)?;
    let mut report = TaskReport {
        load_s: since(load),
        ..TaskReport::default()
    };
    let started = Instant::now();
    let transcript = asr::transcribe_plan(engine.as_mut(), &job.work.mix(), "mix", &plan, progress)
        .map_err(|e| PipelineError::new("Whisper", e))?;
    report.process_s = since(started);
    report.note("words", transcript.words().count());
    work_dir::write_json(&job.work.asr("whisper"), &transcript)?;
    Ok(report)
}

pub(super) fn diff_sheet(job: &Job) -> Result<TaskReport> {
    let parakeet: EngineTranscript = work_dir::read_json(&job.work.asr("parakeet"))?;
    let whisper: EngineTranscript = work_dir::read_json(&job.work.asr("whisper"))?;
    let started = Instant::now();
    let utterances = sheet::build(&parakeet, &[&whisper], &["P", "W"]);
    let mut report = TaskReport {
        process_s: since(started),
        ..TaskReport::default()
    };
    report.note("utterances", utterances.len());
    let locked: usize = utterances
        .iter()
        .flat_map(|u| &u.locked)
        .filter(|l| **l)
        .count();
    let words: usize = utterances.iter().map(|u| u.locked.len()).sum();
    report.note(
        "locked_share",
        format!("{:.3}", locked as f64 / words.max(1) as f64),
    );
    let text: String = utterances.iter().map(|u| format!("{}\n", u.line)).collect();
    work_dir::write_json(&job.work.sheet(), &utterances)?;
    work_dir::write_text(&job.work.sheet_text(), &text)?;
    Ok(report)
}

pub(super) fn redecode_parakeet(job: &Job, progress: &dyn Fn(usize, usize)) -> Result<TaskReport> {
    redecode_with(job, "parakeet", progress, &|| {
        Ok(Box::new(parakeet(job)?) as Box<dyn SpeechEngine>)
    })
}

pub(super) fn redecode_whisper(job: &Job, progress: &dyn Fn(usize, usize)) -> Result<TaskReport> {
    redecode_with(job, "whisper", progress, &|| whisper(job))
}

/// Hear every unsure utterance again on the vocal stem; no model is loaded when there is none.
fn redecode_with(
    job: &Job,
    engine_name: &str,
    progress: &dyn Fn(usize, usize),
    open: &dyn Fn() -> Result<Box<dyn SpeechEngine>>,
) -> Result<TaskReport> {
    let first: AdjudicationPass = work_dir::read_json(&job.work.first_pass())?;
    let utterances: Vec<job_model::outputs::Utterance> = work_dir::read_json(&job.work.sheet())?;
    let duration = job.probe()?.probe.duration_s;
    let ids = redecode::unsure_ids(&first.lines);
    let mut report = TaskReport::default();
    report.note("unsure", ids.len());
    let mut output = Redecode {
        ids: ids.clone(),
        transcript: EngineTranscript {
            engine: engine_name.to_string(),
            input: "vocals".into(),
            chunks: vec![],
        },
    };
    if !ids.is_empty() {
        let plan = SpeechPlan {
            chunks: redecode::spans(&utterances, &ids, duration),
            ..SpeechPlan::default()
        };
        let load = Instant::now();
        let mut engine = open()?;
        report.load_s = since(load);
        let started = Instant::now();
        output.transcript = asr::transcribe_plan(
            engine.as_mut(),
            &job.work.vocals(),
            "vocals",
            &plan,
            progress,
        )
        .map_err(|e| PipelineError::new(engine_name.to_string(), e))?;
        report.process_s = since(started);
    }
    work_dir::write_json(&job.work.redecode(engine_name), &output)?;
    Ok(report)
}

fn parakeet(job: &Job) -> Result<ParakeetTdt> {
    ParakeetTdt::open(&job.models()?.join(parakeet_tdt::MODEL), Device::Cuda)
        .context("load Parakeet")
}

/// Whisper's model folder and file.
pub(super) fn whisper_model(model: WhisperModel) -> (&'static str, &'static str) {
    match model {
        WhisperModel::LargeV3 => ("whisper-large-v3", "ggml-large-v3.bin"),
        WhisperModel::LargeV3Turbo => ("whisper-large-v3-turbo", "ggml-large-v3-turbo-q8_0.bin"),
    }
}

#[cfg(feature = "crispasr")]
fn whisper(job: &Job) -> Result<Box<dyn SpeechEngine>> {
    let (folder, file) = whisper_model(job.settings().whisper);
    let engine = inference::ggml::crispasr::Whisper::open(
        &job.models()?.join(folder).join(file),
        folder,
        WHISPER_THREADS,
    )
    .map_err(|e| PipelineError::new("load Whisper", e))?;
    Ok(Box::new(engine))
}

#[cfg(not(feature = "crispasr"))]
fn whisper(job: &Job) -> Result<Box<dyn SpeechEngine>> {
    let (folder, _) = whisper_model(job.settings().whisper);
    Err(PipelineError::new(
        format!("load {folder}"),
        "this binary is built without CrispASR; Whisper runs in `tbd-subtitles-ggml`",
    ))
}
