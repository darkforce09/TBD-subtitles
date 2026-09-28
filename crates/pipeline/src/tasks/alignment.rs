//! The alignment task: the final text timed against the vocal stem.
//!
//! **Role:** run Parakeet-CTC over each block of the vocal stem and CTC Viterbi over its grid, with
//! the fallbacks of `stages::alignment::run`, and count the words per timing source; each
//! utterance is recognised where any engine heard the words its final line shows.
//!
//! **Position:** called by `tasks::run` inside a worker of the main binary (ONNX Runtime).
//!
//! **Signals and state:** reads `sheet.json`, `adjudicated.json`, `probe.json`, both engines'
//! transcripts and the vocal stem; writes `aligned.json`.
//!
//! **Invariants:** the model loads once per job; a job where every aligner call failed is an error,
//! not a job timed from the backbone alone; the heard spans are cut like the sheet, one per
//! utterance, and a sheet they do not match keeps the backbone's windows.

use std::path::PathBuf;
use std::time::Instant;

use inference::onnx::Device;
use inference::onnx::parakeet_ctc::{self, ParakeetCtc};
use job_model::outputs::{AdjudicationPass, EngineTranscript, TimeSpan, TimingSource, Utterance};
use media_io::pcm_stream::read_f32_range;
use stages::alignment::run::{self, WordAligner};
use stages::alignment::{self, WordTimes, blocks};
use stages::diff_sheet::sheet;

use super::{Job, TaskReport, since};
use crate::error::{Context, Result};
use crate::work_dir;

/// Samples per second of the stems.
const RATE: f64 = 16_000.0;

/// Parakeet-CTC over spans of the vocal stem.
pub(super) struct CtcAligner {
    model: ParakeetCtc,
    vocals: PathBuf,
    pub(super) model_s: f64,
}

impl CtcAligner {
    /// Parakeet-CTC from the job's models folder on `device`, over the job's vocal stem.
    pub(super) fn open(job: &Job, device: Device) -> Result<CtcAligner> {
        let model = ParakeetCtc::open(&job.models()?.join(parakeet_ctc::MODEL), device)
            .context("load Parakeet-CTC")?;
        Ok(CtcAligner {
            model,
            vocals: job.work.vocals(),
            model_s: 0.0,
        })
    }
}

impl WordAligner for CtcAligner {
    fn align(
        &mut self,
        span: TimeSpan,
        words: &[String],
    ) -> std::result::Result<Option<WordTimes>, String> {
        let start = (span.start_s * RATE) as u64;
        let count = (span.duration_s() * RATE) as usize;
        let samples = read_f32_range(&self.vocals, start, count).map_err(|e| e.to_string())?;
        let started = Instant::now();
        let grid = self.model.log_probs(&samples).map_err(|e| e.to_string())?;
        self.model_s += since(started);
        let model = &self.model;
        let times = alignment::align_words_ctc(
            &grid.log_probs,
            grid.vocab,
            parakeet_ctc::BLANK,
            parakeet_ctc::FRAME_S,
            words,
            |w| model.tokens(w).map_err(|e| e.to_string()),
        )?;
        Ok(times.map(|t| {
            t.into_iter()
                .map(|w| w.map(|(a, b)| (a + span.start_s, b + span.start_s)))
                .collect()
        }))
    }
}

/// Where any engine heard each utterance of `sheet`, cut from the transcripts as the diff-sheet
/// step cut them: Parakeet as the backbone, Whisper, when its transcript is there, beside it.
/// Empty, so every window stays the backbone's, when the cut does not match the sheet.
pub(super) fn heard_spans(job: &Job, sheet: &[Utterance]) -> Result<Vec<(f64, f64)>> {
    let parakeet: EngineTranscript = work_dir::read_json(&job.work.asr("parakeet"))?;
    let whisper: Option<EngineTranscript> = work_dir::read_json(&job.work.asr("whisper")).ok();
    let spans = sheet::heard_spans(&parakeet, &whisper.iter().collect::<Vec<_>>());
    if spans.len() != sheet.len() {
        tracing::warn!(
            "the transcripts cut {} utterances, the sheet holds {}; aligning in the backbone's windows",
            spans.len(),
            sheet.len()
        );
        return Ok(Vec::new());
    }
    Ok(spans)
}

pub(super) fn alignment(job: &Job, progress: &dyn Fn(usize, usize)) -> Result<TaskReport> {
    let sheet: Vec<Utterance> = work_dir::read_json(&job.work.sheet())?;
    let adjudicated: AdjudicationPass = work_dir::read_json(&job.work.adjudicated())?;
    let duration = job.probe()?.probe.duration_s;
    let spans = heard_spans(job, &sheet)?;
    let kept = blocks::kept(&sheet, &adjudicated.lines, &spans);
    let load = Instant::now();
    let mut aligner = CtcAligner::open(job, Device::Cuda)?;
    let mut report = TaskReport {
        load_s: since(load),
        ..TaskReport::default()
    };
    let started = Instant::now();
    let aligned = run::align_all(&kept, duration, &mut aligner, progress);
    report.process_s = since(started);
    let words: Vec<_> = aligned.utterances.iter().flat_map(|u| &u.words).collect();
    for source in TimingSource::ALL {
        report.note(
            &format!("words_{}", source.as_str()),
            words.iter().filter(|w| w.source == source).count(),
        );
    }
    report.note("blocks", aligned.blocks);
    report.note("failed_blocks", aligned.failed_blocks);
    report.note("errors", aligned.errors.len());
    report.note("model_s", format!("{:.1}", aligner.model_s));
    if let Some(offset) = aligned.offset_s {
        report.note("offset_ms", format!("{:+.1}", offset * 1000.0));
    }
    if !kept.is_empty() && aligned.errors.len() >= aligned.blocks + kept.len() {
        return Err(crate::error::PipelineError::new(
            "alignment",
            format!("every call failed: {:?}", aligned.errors.first()),
        ));
    }
    work_dir::write_json(&job.work.aligned(), &aligned)?;
    Ok(report)
}
