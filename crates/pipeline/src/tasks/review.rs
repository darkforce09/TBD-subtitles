//! The review task: the owner's corrections timed again, each alone, and every other line's times
//! kept.
//!
//! **Role:** apply the owner's stored line corrections to the final text, time each corrected
//! utterance alone with Parakeet-CTC on the CPU (falling back to its earlier aligned times where
//! its words stayed, then to the backbone's times), leave every other aligned utterance as it
//! is, and store the result (`outputs/review`).
//!
//! **Position:** called by `tasks::run` inside a worker of the main binary (ONNX Runtime on the
//! CPU, so it runs beside a job that holds the GPU).
//!
//! **Signals and state:** reads the vocal stem and, through the step's `StepIo`, the aligned
//! words, the line corrections (`corrections/lines`), the probe, the sheet, the adjudicated lines
//! and both engines' transcripts (for where each line was heard); stores `outputs/review`.
//!
//! **Invariants:** with no corrections, the review is the alignment and no model loads; an
//! uncorrected utterance keeps its words and times exactly; a line the owner drops leaves no
//! words, and a line the owner brings back is timed like a corrected one.

use std::collections::HashMap;
use std::time::Instant;

use inference::onnx::Device;
use job_model::StepName;
use job_model::outputs::{
    AdjudicationPass, Aligned, AlignedUtterance, Corrections, Line, Utterance,
};
use stages::alignment::blocks;
use stages::alignment::run::realign_utterance;

use super::alignment::{CtcAligner, heard_spans};
use super::{Job, StepIo, StepProgress, TaskReport, since};
use crate::error::Result;

pub(super) fn review(job: &Job, io: &mut StepIo, _progress: StepProgress) -> Result<TaskReport> {
    let aligned: Aligned = io.get(StepName::Alignment, None)?;
    let corrections: Corrections = io.corrections()?;
    let mut report = TaskReport::default();
    report.note("corrections", corrections.lines.len());
    if corrections.lines.is_empty() {
        io.put(StepName::Review, None, &aligned)?;
        return Ok(report);
    }
    let sheet: Vec<Utterance> = io.get(StepName::DiffSheet, None)?;
    let adjudicated: AdjudicationPass = io.get(StepName::Readjudicate, None)?;
    let duration = io.probe()?.probe.duration_s;
    let lines = corrected_lines(&adjudicated.lines, &corrections);
    let spans = heard_spans(io, &sheet)?;
    let kept = blocks::kept(&sheet, &lines, &spans);
    let load = Instant::now();
    let mut aligner = CtcAligner::open(job, Device::Cpu)?;
    report.load_s = since(load);
    let started = Instant::now();
    let before: HashMap<&str, &AlignedUtterance> = aligned
        .utterances
        .iter()
        .map(|u| (u.id.as_str(), u))
        .collect();
    let mut errors = aligned.errors.clone();
    let mut utterances = Vec::with_capacity(kept.len());
    let mut retimed = 0;
    for (index, k) in kept.iter().enumerate() {
        match (corrections.get(&k.id), before.get(k.id.as_str())) {
            (None, Some(kept_as_it_was)) => utterances.push((*kept_as_it_was).clone()),
            _ => {
                utterances.push(realign_utterance(
                    &kept,
                    index,
                    duration,
                    &mut aligner,
                    &mut errors,
                    before.get(k.id.as_str()).copied(),
                ));
                retimed += 1;
            }
        }
    }
    report.process_s = since(started);
    report.note("retimed", retimed);
    report.note("model_s", format!("{:.1}", aligner.model_s));
    let reviewed = Aligned {
        utterances,
        errors,
        ..aligned
    };
    io.put(StepName::Review, None, &reviewed)?;
    Ok(report)
}

/// The final lines with each correction's text and flags in place of the model's.
pub(crate) fn corrected_lines(lines: &[Line], corrections: &Corrections) -> Vec<Line> {
    lines
        .iter()
        .map(|line| match corrections.get(&line.id) {
            Some(c) => Line {
                id: line.id.clone(),
                t: c.text.clone(),
                f: c.flags.clone(),
            },
            None => line.clone(),
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/review.rs"]
mod tests;
