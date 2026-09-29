//! Tasks for the six resumable visual steps of a normal video job.
//!
//! **Role:** load models only in their dedicated workers and install typed visual artifacts.
//! **Position:** pipeline task dispatch above the on-screen stages.
//! **Signals and state:** visual JSON, representative crops, translations and ASS events.
//! **Invariants:** disabled visual jobs need no visual model; each output is written atomically.

use super::{Job, StepProgress, TaskReport, since};
use crate::error::{Context, PipelineError, Result};
use crate::work_dir;
use inference::ocr::{OcrDetector, OcrReader};
use job_model::StepName;
use job_model::onscreen::{TextCorrections, TextDocument};
use job_model::outputs::ShotChanges;
use stages::onscreen_text;
use std::time::Instant;

pub(super) fn run(step: StepName, job: &Job, progress: StepProgress) -> Result<TaskReport> {
    let started = Instant::now();
    let mut report = TaskReport::default();
    if !job.settings().onscreen_text.enabled {
        work_dir::write_json(&job.work.text(step), &TextDocument::default())?;
        if step == StepName::TextTypeset {
            work_dir::write_text(&job.work.text_ass(), "")?;
        }
        report.note("disabled", true);
        return Ok(report);
    }
    let probe = job.probe()?;
    let stream =
        probe.probe.video.as_ref().ok_or_else(|| {
            PipelineError::new("visual translation", "the file has no video stream")
        })?;
    let programs = media_io::Programs::beside_current_exe();
    let upstream = match step {
        StepName::TextRead => StepName::TextDetect,
        StepName::TextTrack => StepName::TextRead,
        StepName::TextTranslate => StepName::TextTrack,
        StepName::TextReview => StepName::TextTranslate,
        StepName::TextTypeset => StepName::TextReview,
        _ => StepName::TextDetect,
    };
    let mut document = if step == StepName::TextDetect {
        TextDocument::default()
    } else {
        work_dir::read_json(&job.work.text(upstream))?
    };
    match step {
        StepName::TextDetect => {
            let mut detector = OcrDetector::open(&job.models()?)
                .context("open PP-OCRv5 detector; download visual models in Settings")?;
            report.load_s = since(started);
            let shots: ShotChanges = work_dir::read_json(&job.work.shots())?;
            let estimated_frames =
                (probe.probe.duration_s * stream.fps().unwrap_or(24.0)).ceil() as usize;
            let advance = |done, total| {
                progress(
                    done,
                    if total == 0 {
                        estimated_frames.max(done)
                    } else {
                        total
                    },
                )
            };
            document = onscreen_text::detect::scan(
                &programs,
                &job.video(),
                stream,
                &shots,
                job.work.root(),
                &mut detector,
                &advance,
            )
            .context("scan visible writing")?;
        }
        StepName::TextRead => {
            let mut reader = OcrReader::open(&job.models()?)
                .context("open local text readers; download visual models in Settings")?;
            report.load_s = since(started);
            onscreen_text::read::read(
                &mut document,
                job.work.root(),
                &mut reader,
                &corrections(job)?,
                progress,
            )
            .context("read visible writing")?;
        }
        StepName::TextTrack => {
            onscreen_text::track::track(&mut document, &programs, &job.video(), stream, progress)
                .context("track visible writing")?
        }
        StepName::TextTranslate => translate(job, &mut document, progress, &mut report)?,
        StepName::TextReview => {
            onscreen_text::review::apply(&mut document, &corrections(job)?, probe.probe.duration_s)
                .context("apply visual corrections")?
        }
        StepName::TextTypeset => {
            let events = onscreen_text::typeset::events(&mut document)
                .context("typeset visible translations")?;
            work_dir::write_text(&job.work.text_ass(), &events)?;
        }
        _ => return Err(PipelineError::new("visual step", "unexpected task")),
    }
    report.process_s = (since(started) - report.load_s).max(0.0);
    let summary = document.summary();
    report.note("detected", summary.detected);
    report.note("translated", summary.translated);
    report.note("fallback", summary.fallback);
    report.note("unresolved", summary.unresolved);
    report.note("flagged", summary.flagged);
    work_dir::write_json(&job.work.text(step), &document)?;
    Ok(report)
}

fn corrections(job: &Job) -> Result<TextCorrections> {
    if job.work.text_corrections().exists() {
        work_dir::read_json(&job.work.text_corrections())
    } else {
        Ok(TextCorrections::default())
    }
}

#[cfg(feature = "mistralrs")]
fn translate(
    job: &Job,
    document: &mut TextDocument,
    progress: StepProgress,
    report: &mut TaskReport,
) -> Result<()> {
    use inference::llm::{claude_cli::ClaudeCli, mistral_rs::MistralRs};
    use onscreen_text::translate::{TranslationInput, finish, prepare};
    let started = Instant::now();
    let mut local = MistralRs::open(&job.models()?.join("qwen3.5-4b"), "Qwen3.5-4B-Q4_K_M.gguf")
        .context("load local translation model")?;
    local.max_tokens = 512;
    report.load_s = since(started);
    let mut claude = ClaudeCli::new(&job.settings().llm_model, job.work.claude_cwd());
    let dialogue = work_dir::read_json(&job.work.cues())?;
    let corrections = corrections(job)?;
    let own_output = job.video().with_extension("ass");
    let input = TranslationInput {
        root: job.work.root(),
        dialogue: &dialogue,
        glossary: &job.settings().glossary,
        settings: &job.settings().onscreen_text,
        corrections: &corrections,
        excluded_reference: Some(&own_output),
    };
    let prepared = prepare(document, &input, &mut local, progress)
        .context("translate visible Japanese locally")?;
    drop(local);
    finish(document, &input, prepared, Some(&mut claude), progress)
        .context("verify uncertain visible Japanese")
}

#[cfg(not(feature = "mistralrs"))]
fn translate(
    _job: &Job,
    _document: &mut TextDocument,
    _progress: StepProgress,
    _report: &mut TaskReport,
) -> Result<()> {
    Err(PipelineError::new(
        "local translation",
        "this worker has no mistral.rs backend; build tbd-subtitles-llm with --features mistralrs or reinstall the AppImage",
    ))
}
