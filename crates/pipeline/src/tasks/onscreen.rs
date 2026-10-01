//! Tasks for the six resumable visual steps of a normal video job.
//!
//! **Role:** load models only in their dedicated workers and store each step's typed text
//! document.
//! **Position:** pipeline task dispatch above the on-screen stages; every stored input and output
//! goes through the step's `StepIo`.
//! **Signals and state:** reads the probe, the shot changes, the cue track, the owner's text
//! corrections (`corrections/text`) and the previous visual step's document; stores
//! `outputs/<step>` and, for typesetting, the ASS events as `outputs/text_typeset/ass`; writes
//! representative crops and keyframe stills under `visual/`, and the readings, translation and
//! Claude caches; the translation looks its occurrences up in the sign library.
//! **Invariants:** disabled visual jobs need no visual model and store empty documents; every PNG
//! a document names is synced before the document is handed to the store; the local translation
//! model loads only when Claude and the sign library leave an occurrence unanswered; a sign the
//! library holds for an occurrence of another job is its translation, and its keyframe asks
//! Claude only when it shows writing the library does not hold.

use super::{Job, StepIo, StepProgress, TaskReport, since};
use crate::error::{Context, PipelineError, Result};
use crate::library::signs;
use crate::work_dir::store::keys;
use inference::ocr::{OcrDetector, OcrReader, TextDetection};
use job_model::StepName;
use job_model::onscreen::TextDocument;
use job_model::outputs::{ProbeDecoded, ShotChanges, VideoStream};
use stages::onscreen_text;
use std::time::Instant;

pub(super) fn run(
    step: StepName,
    job: &Job,
    io: &mut StepIo,
    progress: StepProgress,
) -> Result<TaskReport> {
    let started = Instant::now();
    let mut report = TaskReport::default();
    if !job.settings().onscreen_text.enabled {
        io.put(step, None, &TextDocument::default())?;
        if step == StepName::TextTypeset {
            io.put(step, Some(keys::TYPESET_ASS), &String::new())?;
        }
        report.note("disabled", true);
        return Ok(report);
    }
    let mut document = match upstream(step) {
        Some(upstream) => io.get(upstream, None)?,
        None => TextDocument::default(),
    };
    match step {
        StepName::TextDetect => {
            let probe = io.probe()?;
            let stream = video_stream(&probe)?;
            let programs = media_io::Programs::beside_current_exe();
            let mut detector = OcrDetector::open(&job.models()?)
                .context("open PP-OCRv5 detector; download visual models in Settings")?;
            report.load_s = since(started);
            let shots: ShotChanges = io.get(StepName::ShotScan, None)?;
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
            let mut source =
                onscreen_text::detect::FfmpegSource::open(&programs, &job.video(), stream)
                    .context("open the proxy frame stream")?;
            document = onscreen_text::detect::scan(
                &mut source,
                stream,
                &shots,
                job.work.root(),
                &mut detector as &mut dyn TextDetection,
                &advance,
            )
            .context("scan visible writing")?;
        }
        StepName::TextRead => {
            let mut reader = OcrReader::open(&job.models()?)
                .context("open local text readers; download visual models in Settings")?;
            report.load_s = since(started);
            let shots: ShotChanges = io.get(StepName::ShotScan, None)?;
            onscreen_text::read::read(
                &mut document,
                job.work.root(),
                &mut reader,
                &io.text_corrections()?,
                &shots,
                progress,
            )
            .context("read visible writing")?;
        }
        StepName::TextTrack => {
            let probe = io.probe()?;
            onscreen_text::track::track(&mut document, video_stream(&probe)?, progress)
                .context("check visible writing geometry")?
        }
        StepName::TextTranslate => {
            let known = match &job.library {
                Some(library) => signs::matches(library, &document, job.work.root(), &job.id())?,
                None => signs::Matches::new(),
            };
            if !known.is_empty() {
                report.note("library_matches", known.len());
            }
            translate(job, io, &mut document, &known, progress, &mut report)?
        }
        StepName::TextReview => {
            let duration_s = io.probe()?.probe.duration_s;
            onscreen_text::review::apply(&mut document, &io.text_corrections()?, duration_s)
                .context("apply visual corrections")?;
            let shots: ShotChanges = io.get(StepName::ShotScan, None)?;
            onscreen_text::unify::unify(&mut document, &shots);
        }
        StepName::TextTypeset => {
            let events = onscreen_text::typeset::events(&mut document)
                .context("typeset visible translations")?;
            io.put(step, Some(keys::TYPESET_ASS), &events)?;
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
    io.put(step, None, &document)?;
    Ok(report)
}

/// The visual step whose document `step` continues; `None` for detection, which starts one.
fn upstream(step: StepName) -> Option<StepName> {
    match step {
        StepName::TextRead => Some(StepName::TextDetect),
        StepName::TextTrack => Some(StepName::TextRead),
        StepName::TextTranslate => Some(StepName::TextTrack),
        StepName::TextReview => Some(StepName::TextTranslate),
        StepName::TextTypeset => Some(StepName::TextReview),
        _ => None,
    }
}

/// The probed video stream; a file without one has no writing to find.
fn video_stream(probe: &ProbeDecoded) -> Result<&VideoStream> {
    probe
        .probe
        .video
        .as_ref()
        .ok_or_else(|| PipelineError::new("visual translation", "the file has no video stream"))
}

#[cfg(feature = "mistralrs")]
fn translate(
    job: &Job,
    io: &StepIo,
    document: &mut TextDocument,
    known: &signs::Matches,
    progress: StepProgress,
    report: &mut TaskReport,
) -> Result<()> {
    use inference::llm::{LanguageModel, claude_cli::ClaudeCli, mistral_rs::MistralRs};
    use onscreen_text::translate::{TranslationInput, translate_known};
    use std::cell::Cell;
    use subtitle_formats::cue::CueTrack;
    let models = job.models()?;
    let load_s = Cell::new(0.0);
    // The local model loads only for occurrences Claude leaves, so its load time is measured
    // where it happens.
    let mut open_local = || -> onscreen_text::TextResult<Box<dyn LanguageModel>> {
        let started = Instant::now();
        let mut local = MistralRs::open(&models.join("qwen3.5-4b"), "Qwen3.5-4B-Q4_K_M.gguf")?;
        local.max_tokens = 512;
        load_s.set(since(started));
        Ok(Box::new(local))
    };
    let mut claude = ClaudeCli::new(&job.settings().llm_model, job.work.claude_cwd());
    let dialogue: CueTrack = io.get(StepName::Cues, None)?;
    let shots: ShotChanges = io.get(StepName::ShotScan, None)?;
    let corrections = io.text_corrections()?;
    let own_output = job.video().with_extension("ass");
    let input = TranslationInput {
        root: job.work.root(),
        dialogue: &dialogue,
        cuts: &shots,
        glossary: &job.settings().glossary,
        settings: &job.settings().onscreen_text,
        corrections: &corrections,
        excluded_reference: Some(&own_output),
        parallel_calls: job.settings().llm_processes,
    };
    translate_known(
        document,
        &input,
        known,
        &mut open_local,
        Some(&mut claude),
        progress,
    )
    .context("translate visible Japanese")?;
    report.load_s = load_s.get();
    Ok(())
}

#[cfg(not(feature = "mistralrs"))]
fn translate(
    _job: &Job,
    _io: &StepIo,
    _document: &mut TextDocument,
    _known: &signs::Matches,
    _progress: StepProgress,
    _report: &mut TaskReport,
) -> Result<()> {
    Err(PipelineError::new(
        "local translation",
        "this worker has no mistral.rs backend; build tbd-subtitles-llm with --features mistralrs or reinstall the AppImage",
    ))
}

#[cfg(test)]
#[path = "tests/onscreen.rs"]
mod tests;
