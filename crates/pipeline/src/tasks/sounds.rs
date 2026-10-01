//! The sound tasks: CED over both stems, then the candidates and the language model's choice of
//! sound cues.
//!
//! **Role:** tag sound events on the background and vocal stems; list the candidates (songs,
//! effects, non-speech voices, Whisper's tags); have `claude -p` choose and word the cues.
//!
//! **Position:** called by `tasks::run` inside workers of the main binary; uses
//! `stages::sound_events` and `stages::adjudication::sound_cues`.
//!
//! **Signals and state:** reads the stems and, through the step's `StepIo`, the probe, the sound
//! events, the sheet, the adjudicated lines and Whisper's transcript; stores the sound events
//! (`outputs/sound_events`) and the chosen cues (`outputs/sound_cues`).
//!
//! **Invariants:** no model call is made when there is no candidate; every song ends up with a
//! music cue.

use std::collections::HashMap;
use std::time::Instant;

use inference::onnx::Device;
use inference::onnx::ced::{self, Ced};
use job_model::StepName;
use job_model::outputs::{AdjudicationPass, EngineTranscript, SoundCues, SoundEvent, Utterance};
use stages::adjudication::sound_cues;
use stages::sound_events::{self, Windowing, candidates, classes};

use super::{Job, StepIo, StepProgress, TaskReport, llm, since};
use crate::error::{Context, PipelineError, Result};

pub(super) fn sound_events(
    job: &Job,
    io: &mut StepIo,
    progress: StepProgress,
) -> Result<TaskReport> {
    let duration = io.probe()?.probe.duration_s;
    let load = Instant::now();
    let mut tagger =
        Ced::open(&job.models()?.join(ced::MODEL), Device::Cuda).context("load CED")?;
    let mut report = TaskReport {
        load_s: since(load),
        ..TaskReport::default()
    };
    let started = Instant::now();
    let windowing = Windowing::default();
    let mut events: Vec<SoundEvent> = Vec::new();
    let stems = [
        ("background", job.work.background(), classes::BACKGROUND),
        ("vocals", job.work.vocals(), classes::VOCALS),
    ];
    for (i, (stem, path, table)) in stems.iter().enumerate() {
        let rows = sound_events::score_stem(&mut tagger, path, duration, &windowing)
            .map_err(|e| PipelineError::new(format!("score the {stem} stem"), e))?;
        for rule in classes::rules(table).map_err(|e| PipelineError::new("sound classes", e))? {
            events.extend(sound_events::events(&rows, &rule, &windowing, stem));
        }
        progress(i + 1, stems.len());
    }
    report.process_s = since(started);
    report.note("events", events.len());
    io.put(StepName::SoundEvents, None, &events)?;
    Ok(report)
}

pub(super) fn sound_cues(job: &Job, io: &mut StepIo, progress: StepProgress) -> Result<TaskReport> {
    let events: Vec<SoundEvent> = io.get(StepName::SoundEvents, None)?;
    let sheet: Vec<Utterance> = io.get(StepName::DiffSheet, None)?;
    let adjudicated: AdjudicationPass = io.get(StepName::Readjudicate, None)?;
    let whisper: EngineTranscript = io.get(StepName::AsrWhisper, None)?;
    let started = Instant::now();
    let (found, songs) = candidates::candidates(&events, &sheet, &adjudicated.lines, &whisper);
    let text: HashMap<&str, &job_model::outputs::Line> = adjudicated
        .lines
        .iter()
        .map(|l| (l.id.as_str(), l))
        .collect();
    let dialogue: Vec<(f64, String)> = sheet
        .iter()
        .filter_map(|u| text.get(u.id.as_str()).map(|l| (u, l)))
        .filter(|(_, l)| !l.has_flag("LYRIC") && !l.has_flag("DROP"))
        .map(|(u, l)| (u.start_s, l.t.replace("||", " ")))
        .collect();
    let chosen = if found.is_empty() {
        SoundCues::default()
    } else {
        let make = llm::claude(job);
        sound_cues::choose(
            &make,
            job.settings().llm_processes,
            &found,
            &dialogue,
            &job.glossary(),
            progress,
        )
    };
    let mut report = TaskReport {
        process_s: since(started),
        ..TaskReport::default()
    };
    report.note("candidates", found.len());
    report.note("songs", songs.len());
    report.note("cues", chosen.cues.len());
    report.note("refused", chosen.refused.len());
    report.note("failed_calls", chosen.failed_calls.len());
    report.note("cost_usd", format!("{:.2}", chosen.cost_usd));
    io.put(StepName::SoundCues, None, &chosen)?;
    Ok(report)
}
